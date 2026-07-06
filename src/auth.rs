use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::cache::RedisCache;

pub const MIGRATED_PASSWORD_SCHEME: &str = "argon2id-md5-v1";
pub const MODERN_PASSWORD_SCHEME: &str = "argon2id-v1";
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub name: String,
    pub level: i32,
}

#[derive(Clone)]
pub struct SessionStore {
    entries: Arc<RwLock<HashMap<String, Session>>>,
    redis: Option<RedisCache>,
    ttl: Duration,
    cookie_secure: bool,
}

#[derive(Clone)]
struct Session {
    user: AuthenticatedUser,
    expires_at: Instant,
    expires_at_epoch_ms: u64,
    viewed_post_ids: HashSet<i32>,
}

#[derive(Clone, Deserialize, Serialize)]
struct PersistentSession {
    user: AuthenticatedUser,
    expires_at_epoch_ms: u64,
    #[serde(default)]
    user_session_version: u64,
    #[serde(default)]
    viewed_post_ids: HashSet<i32>,
}

enum RedisSessionLookup {
    Found(PersistentSession),
    Missing,
    Unavailable,
}

impl SessionStore {
    pub fn new(ttl: Duration, cookie_secure: bool) -> Self {
        Self::with_redis(ttl, cookie_secure, None)
    }

    pub fn with_redis(ttl: Duration, cookie_secure: bool, redis: Option<RedisCache>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            redis,
            ttl,
            cookie_secure,
        }
    }

    pub fn create(&self, user: AuthenticatedUser) -> String {
        let token = SaltString::generate(&mut OsRng).to_string();
        let expires_at_epoch_ms = SystemTime::now()
            .checked_add(self.ttl)
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis().try_into().unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX);
        let session = Session {
            user,
            expires_at: Instant::now() + self.ttl,
            expires_at_epoch_ms,
            viewed_post_ids: HashSet::new(),
        };
        self.entries
            .write()
            .expect("session store lock poisoned")
            .insert(token.clone(), session);
        token
    }

    pub async fn create_persistent(&self, user: AuthenticatedUser) -> String {
        let token = self.create(user);
        self.persist_existing_session(&token).await;
        token
    }

    pub fn get(&self, token: &str) -> Option<AuthenticatedUser> {
        let now = Instant::now();
        let mut entries = self.entries.write().expect("session store lock poisoned");
        match entries.get(token) {
            Some(session) if session.expires_at > now => Some(session.user.clone()),
            Some(_) => {
                entries.remove(token);
                None
            }
            None => None,
        }
    }

    pub async fn get_persistent(&self, token: &str) -> Option<AuthenticatedUser> {
        match self.redis_session(token).await {
            RedisSessionLookup::Found(session) => Some(session.user),
            RedisSessionLookup::Unavailable => self.get(token),
            RedisSessionLookup::Missing => None,
        }
    }

    pub fn remove(&self, token: &str) {
        self.entries
            .write()
            .expect("session store lock poisoned")
            .remove(token);
    }

    pub async fn remove_persistent(&self, token: &str) {
        self.remove(token);
        let Some(redis) = self.redis.as_ref().filter(|redis| redis.is_enabled()) else {
            return;
        };
        let session = redis
            .get_json::<PersistentSession>(&session_key(token))
            .await
            .ok()
            .flatten();
        if let Some(session) = session {
            if let Err(error) = redis
                .remove_set_member(&user_sessions_key(session.user.id), token)
                .await
            {
                tracing::warn!(?error, "failed to remove session from Redis user index");
            }
        }
        if let Err(error) = redis.delete(&session_key(token)).await {
            tracing::warn!(?error, "failed to remove Redis session");
        }
    }

    pub fn expires_at_epoch_ms(&self, token: &str) -> Option<u64> {
        let now = Instant::now();
        let mut entries = self.entries.write().expect("session store lock poisoned");
        match entries.get(token) {
            Some(session) if session.expires_at > now => Some(session.expires_at_epoch_ms),
            Some(_) => {
                entries.remove(token);
                None
            }
            None => None,
        }
    }

    pub async fn persistent_expires_at_epoch_ms(&self, token: &str) -> Option<u64> {
        match self.redis_session(token).await {
            RedisSessionLookup::Found(session) => Some(session.expires_at_epoch_ms),
            RedisSessionLookup::Unavailable => self.expires_at_epoch_ms(token),
            RedisSessionLookup::Missing => None,
        }
    }

    pub fn mark_post_viewed(&self, token: &str, post_id: i32) -> bool {
        self.mark_post_viewed_in_memory(token, post_id)
            .unwrap_or(false)
    }

    pub async fn mark_post_viewed_persistent(&self, token: &str, post_id: i32) -> bool {
        let Some(redis) = self.redis.as_ref().filter(|redis| redis.is_enabled()) else {
            return self.mark_post_viewed(token, post_id);
        };

        let session = match self.redis_session(token).await {
            RedisSessionLookup::Found(session) => session,
            RedisSessionLookup::Missing => return false,
            RedisSessionLookup::Unavailable => return self.mark_post_viewed(token, post_id),
        };
        let ttl = ttl_until(session.expires_at_epoch_ms);
        match redis
            .add_set_member(&session_viewed_key(token), &post_id.to_string(), ttl)
            .await
        {
            Ok(inserted) => {
                self.record_post_viewed_in_memory(token, post_id);
                if inserted {
                    let mut session = session;
                    session.viewed_post_ids.insert(post_id);
                    if let Err(error) = redis
                        .set_json_with_ttl(&session_key(token), &session, ttl)
                        .await
                    {
                        tracing::warn!(?error, "failed to update Redis session viewed posts");
                    }
                }
                inserted
            }
            Err(error) => {
                tracing::warn!(?error, "failed to mark Redis session viewed post");
                self.mark_post_viewed(token, post_id)
            }
        }
    }

    fn mark_post_viewed_in_memory(&self, token: &str, post_id: i32) -> Option<bool> {
        let now = Instant::now();
        let mut entries = self.entries.write().expect("session store lock poisoned");
        match entries.get_mut(token) {
            Some(session) if session.expires_at > now => {
                Some(session.viewed_post_ids.insert(post_id))
            }
            Some(_) => {
                entries.remove(token);
                Some(false)
            }
            None => None,
        }
    }

    pub fn remove_user(&self, user_id: i32) {
        self.entries
            .write()
            .expect("session store lock poisoned")
            .retain(|_, session| session.user.id != user_id);
    }

    pub async fn remove_user_persistent(&self, user_id: i32) -> redis::RedisResult<()> {
        self.remove_user(user_id);
        let Some(redis) = self.redis.as_ref().filter(|redis| redis.is_enabled()) else {
            return Ok(());
        };
        redis
            .increment_with_ttl(&user_session_version_key(user_id), self.ttl)
            .await?;
        let tokens = match redis.set_members(&user_sessions_key(user_id)).await {
            Ok(tokens) => tokens,
            Err(error) => {
                tracing::warn!(?error, "failed to read Redis user session index");
                return Ok(());
            }
        };
        for token in tokens {
            if let Err(error) = redis.delete(&session_key(&token)).await {
                tracing::warn!(?error, "failed to remove Redis session for user");
            }
            if let Err(error) = redis.delete(&session_viewed_key(&token)).await {
                tracing::warn!(?error, "failed to remove Redis session viewed-post set");
            }
        }
        if let Err(error) = redis.delete(&user_sessions_key(user_id)).await {
            tracing::warn!(?error, "failed to remove Redis user session index");
        }
        Ok(())
    }

    pub fn max_age_seconds(&self) -> u64 {
        self.ttl.as_secs()
    }

    pub fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }

    async fn persist_existing_session(&self, token: &str) {
        let Some(redis) = self.redis.as_ref().filter(|redis| redis.is_enabled()) else {
            return;
        };
        let Some(session) = self.memory_session(token) else {
            return;
        };
        let mut persistent = PersistentSession::from(session);
        persistent.user_session_version = match redis
            .get_counter(&user_session_version_key(persistent.user.id))
            .await
        {
            Ok(version) => version,
            Err(error) => {
                tracing::warn!(?error, "failed to read Redis user session version");
                return;
            }
        };
        if persistent.user_session_version > 0
            && let Err(error) = redis
                .expire(&user_session_version_key(persistent.user.id), self.ttl)
                .await
        {
            tracing::warn!(?error, "failed to refresh Redis user session version TTL");
            return;
        }
        let ttl = ttl_until(persistent.expires_at_epoch_ms);
        if let Err(error) = redis
            .set_json_with_ttl(&session_key(token), &persistent, ttl)
            .await
        {
            tracing::warn!(?error, "failed to persist session in Redis");
            return;
        }
        if let Err(error) = redis
            .add_set_member(&user_sessions_key(persistent.user.id), token, self.ttl)
            .await
        {
            tracing::warn!(?error, "failed to index Redis session by user");
        }
    }

    async fn redis_session(&self, token: &str) -> RedisSessionLookup {
        let Some(redis) = self.redis.as_ref().filter(|redis| redis.is_enabled()) else {
            return RedisSessionLookup::Unavailable;
        };
        match redis
            .get_json::<PersistentSession>(&session_key(token))
            .await
        {
            Ok(Some(session)) if session.expires_at_epoch_ms > current_epoch_ms() => {
                match redis
                    .get_counter(&user_session_version_key(session.user.id))
                    .await
                {
                    Ok(version) if version == session.user_session_version => {}
                    Ok(_) => {
                        self.remove(token);
                        return RedisSessionLookup::Missing;
                    }
                    Err(error) => {
                        tracing::warn!(?error, "failed to read Redis user session version");
                        return RedisSessionLookup::Unavailable;
                    }
                }
                self.insert_memory_session(token.to_string(), session.clone());
                RedisSessionLookup::Found(session)
            }
            Ok(Some(_)) => {
                self.remove_persistent(token).await;
                RedisSessionLookup::Missing
            }
            Ok(None) => {
                self.remove(token);
                RedisSessionLookup::Missing
            }
            Err(error) => {
                tracing::warn!(?error, "failed to read Redis session");
                RedisSessionLookup::Unavailable
            }
        }
    }

    fn record_post_viewed_in_memory(&self, token: &str, post_id: i32) {
        let _ = self.mark_post_viewed_in_memory(token, post_id);
    }

    fn memory_session(&self, token: &str) -> Option<Session> {
        let now = Instant::now();
        let mut entries = self.entries.write().expect("session store lock poisoned");
        match entries.get(token) {
            Some(session) if session.expires_at > now => Some(session.clone()),
            Some(_) => {
                entries.remove(token);
                None
            }
            None => None,
        }
    }

    fn insert_memory_session(&self, token: String, session: PersistentSession) {
        let now_epoch_ms = current_epoch_ms();
        if session.expires_at_epoch_ms <= now_epoch_ms {
            return;
        }
        let ttl = Duration::from_millis(session.expires_at_epoch_ms - now_epoch_ms);
        self.entries
            .write()
            .expect("session store lock poisoned")
            .insert(
                token,
                Session {
                    user: session.user,
                    expires_at: Instant::now() + ttl,
                    expires_at_epoch_ms: session.expires_at_epoch_ms,
                    viewed_post_ids: session.viewed_post_ids,
                },
            );
    }
}

impl From<Session> for PersistentSession {
    fn from(session: Session) -> Self {
        Self {
            user: session.user,
            expires_at_epoch_ms: session.expires_at_epoch_ms,
            user_session_version: 0,
            viewed_post_ids: session.viewed_post_ids,
        }
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn ttl_until(expires_at_epoch_ms: u64) -> Duration {
    let remaining_ms = expires_at_epoch_ms.saturating_sub(current_epoch_ms());
    Duration::from_secs(remaining_ms.saturating_add(999) / 1000).max(Duration::from_secs(1))
}

fn session_key(token: &str) -> String {
    format!("session:{token}")
}

fn session_viewed_key(token: &str) -> String {
    format!("session_viewed:{token}")
}

fn user_sessions_key(user_id: i32) -> String {
    format!("session_user:{user_id}")
}

fn user_session_version_key(user_id: i32) -> String {
    format!("session_user_version:{user_id}")
}

pub fn configured_argon2id() -> anyhow::Result<Argon2<'static>> {
    let parameters = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        None,
    )
    .context("invalid Argon2id configuration")?;

    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters))
}

pub fn legacy_password_input(raw_password: &str) -> String {
    format!("{:x}", Md5::digest(raw_password.as_bytes()))
}

pub fn verify_migrated_password(raw_password: &str, encoded_hash: &str) -> bool {
    verify_encoded_password(legacy_password_input(raw_password).as_bytes(), encoded_hash)
}

pub fn verify_modern_password(raw_password: &str, encoded_hash: &str) -> bool {
    verify_encoded_password(raw_password.as_bytes(), encoded_hash)
}

fn verify_encoded_password(input: &[u8], encoded_hash: &str) -> bool {
    let Ok(hash) = PasswordHash::new(encoded_hash) else {
        return false;
    };
    let Ok(argon2) = configured_argon2id() else {
        return false;
    };
    argon2.verify_password(input, &hash).is_ok()
}

pub fn hash_migrated_input(md5_hash: &str) -> anyhow::Result<String> {
    hash_password_input(md5_hash.as_bytes())
}

pub fn hash_modern_password(raw_password: &str) -> anyhow::Result<String> {
    hash_password_input(raw_password.as_bytes())
}

fn hash_password_input(input: &[u8]) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    configured_argon2id()?
        .hash_password(input, &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow::anyhow!("failed to hash credential: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        AuthenticatedUser, SessionStore, hash_migrated_input, hash_modern_password,
        legacy_password_input, verify_migrated_password, verify_modern_password,
    };
    use std::time::Duration;

    #[test]
    fn migrated_hash_verifies_original_password() {
        let hash = hash_migrated_input(&legacy_password_input("correct password")).unwrap();

        assert!(verify_migrated_password("correct password", &hash));
        assert!(!verify_migrated_password("wrong password", &hash));
    }

    #[test]
    fn modern_hash_verifies_raw_password_only() {
        let hash = hash_modern_password("Modern-password1!").unwrap();

        assert!(verify_modern_password("Modern-password1!", &hash));
        assert!(!verify_migrated_password("Modern-password1!", &hash));
    }

    #[test]
    fn remove_user_invalidates_only_matching_sessions() {
        let sessions = SessionStore::new(Duration::from_secs(60), false);
        let first = sessions.create(AuthenticatedUser {
            id: 1,
            name: "first".to_string(),
            level: 1,
        });
        let second = sessions.create(AuthenticatedUser {
            id: 2,
            name: "second".to_string(),
            level: 1,
        });

        sessions.remove_user(1);

        assert!(sessions.get(&first).is_none());
        assert!(sessions.get(&second).is_some());
    }

    #[test]
    fn mark_post_viewed_tracks_once_per_session() {
        let sessions = SessionStore::new(Duration::from_secs(60), false);
        let first = sessions.create(AuthenticatedUser {
            id: 1,
            name: "first".to_string(),
            level: 1,
        });
        let second = sessions.create(AuthenticatedUser {
            id: 1,
            name: "first".to_string(),
            level: 1,
        });

        assert!(sessions.mark_post_viewed(&first, 101));
        assert!(!sessions.mark_post_viewed(&first, 101));
        assert!(sessions.mark_post_viewed(&first, 102));
        assert!(sessions.mark_post_viewed(&second, 101));
        assert!(!sessions.mark_post_viewed("missing", 101));
    }
}
