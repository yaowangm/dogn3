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
use serde::Serialize;

pub const MIGRATED_PASSWORD_SCHEME: &str = "argon2id-md5-v1";
pub const MODERN_PASSWORD_SCHEME: &str = "argon2id-v1";
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

#[derive(Clone, Debug, Serialize)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub name: String,
    pub level: i32,
}

#[derive(Clone)]
pub struct SessionStore {
    entries: Arc<RwLock<HashMap<String, Session>>>,
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

impl SessionStore {
    pub fn new(ttl: Duration, cookie_secure: bool) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
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

    pub fn remove(&self, token: &str) {
        self.entries
            .write()
            .expect("session store lock poisoned")
            .remove(token);
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

    pub fn mark_post_viewed(&self, token: &str, post_id: i32) -> bool {
        let now = Instant::now();
        let mut entries = self.entries.write().expect("session store lock poisoned");
        match entries.get_mut(token) {
            Some(session) if session.expires_at > now => session.viewed_post_ids.insert(post_id),
            Some(_) => {
                entries.remove(token);
                false
            }
            None => false,
        }
    }

    pub fn remove_user(&self, user_id: i32) {
        self.entries
            .write()
            .expect("session store lock poisoned")
            .retain(|_, session| session.user.id != user_id);
    }

    pub fn max_age_seconds(&self) -> u64 {
        self.ttl.as_secs()
    }

    pub fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }
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
