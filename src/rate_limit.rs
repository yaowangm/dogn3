use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use crate::cache::RedisCache;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RateLimitBackend {
    Redis,
    Memory,
}

#[derive(Clone, Copy, Debug)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub backend: RateLimitBackend,
    pub login_fail_window: Duration,
    pub login_fail_max_per_user: u64,
    pub login_fail_max_per_ip: u64,
    pub login_fail_lock: Duration,
    pub password_reset_window: Duration,
    pub password_reset_max_per_email: u64,
    pub password_reset_max_per_ip: u64,
    pub password_reset_confirm_window: Duration,
    pub password_reset_confirm_max_per_ip: u64,
}

impl RateLimitConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            backend: RateLimitBackend::Memory,
            login_fail_window: Duration::from_secs(900),
            login_fail_max_per_user: 5,
            login_fail_max_per_ip: 30,
            login_fail_lock: Duration::from_secs(900),
            password_reset_window: Duration::from_secs(3600),
            password_reset_max_per_email: 3,
            password_reset_max_per_ip: 20,
            password_reset_confirm_window: Duration::from_secs(900),
            password_reset_confirm_max_per_ip: 20,
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    redis: Option<RedisCache>,
    memory: MemoryRateLimitStore,
}

#[derive(Clone, Default)]
struct MemoryRateLimitStore {
    entries: Arc<Mutex<HashMap<String, MemoryEntry>>>,
}

#[derive(Clone, Copy)]
struct MemoryEntry {
    value: u64,
    expires_at: Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("rate limit redis backend is unavailable")]
    BackendUnavailable,
    #[error("rate limit redis error")]
    Redis(#[from] redis::RedisError),
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig, redis: Option<RedisCache>) -> Self {
        Self {
            config,
            redis,
            memory: MemoryRateLimitStore::default(),
        }
    }

    pub fn disabled() -> Self {
        Self::new(RateLimitConfig::disabled(), None)
    }

    pub async fn login_is_blocked(
        &self,
        name: &str,
        ip: Option<&str>,
    ) -> Result<bool, RateLimitError> {
        if !self.config.enabled {
            return Ok(false);
        }
        Ok(self.exists(&login_user_lock_key(name)).await?
            || self.exists(&login_ip_lock_key(ip)).await?)
    }

    pub async fn record_login_failure(
        &self,
        name: &str,
        ip: Option<&str>,
    ) -> Result<bool, RateLimitError> {
        if !self.config.enabled {
            return Ok(false);
        }

        let mut blocked = false;
        if !name.is_empty() {
            let value = self
                .increment(&login_user_counter_key(name), self.config.login_fail_window)
                .await?;
            if value > self.config.login_fail_max_per_user {
                self.set_lock(&login_user_lock_key(name), self.config.login_fail_lock)
                    .await?;
                blocked = true;
            }
        }
        let value = self
            .increment(&login_ip_counter_key(ip), self.config.login_fail_window)
            .await?;
        if value > self.config.login_fail_max_per_ip {
            self.set_lock(&login_ip_lock_key(ip), self.config.login_fail_lock)
                .await?;
            blocked = true;
        }
        Ok(blocked)
    }

    pub async fn clear_login_user(&self, name: &str) -> Result<(), RateLimitError> {
        if !self.config.enabled || name.is_empty() {
            return Ok(());
        }
        self.delete(&login_user_counter_key(name)).await?;
        self.delete(&login_user_lock_key(name)).await
    }

    pub async fn password_reset_request_is_blocked(
        &self,
        email: &str,
        ip: Option<&str>,
    ) -> Result<bool, RateLimitError> {
        if !self.config.enabled {
            return Ok(false);
        }
        if self.exists(&reset_email_lock_key(email)).await?
            || self.exists(&reset_ip_lock_key(ip)).await?
        {
            return Ok(true);
        }

        let mut blocked = false;
        if !email.is_empty() {
            let value = self
                .increment(
                    &reset_email_counter_key(email),
                    self.config.password_reset_window,
                )
                .await?;
            if value > self.config.password_reset_max_per_email {
                self.set_lock(
                    &reset_email_lock_key(email),
                    self.config.password_reset_window,
                )
                .await?;
                blocked = true;
            }
        }
        let value = self
            .increment(&reset_ip_counter_key(ip), self.config.password_reset_window)
            .await?;
        if value > self.config.password_reset_max_per_ip {
            self.set_lock(&reset_ip_lock_key(ip), self.config.password_reset_window)
                .await?;
            blocked = true;
        }
        Ok(blocked)
    }

    pub async fn password_reset_confirm_is_blocked(
        &self,
        ip: Option<&str>,
    ) -> Result<bool, RateLimitError> {
        if !self.config.enabled {
            return Ok(false);
        }
        self.exists(&reset_confirm_ip_lock_key(ip)).await
    }

    pub async fn record_invalid_password_reset_confirm(
        &self,
        ip: Option<&str>,
    ) -> Result<bool, RateLimitError> {
        if !self.config.enabled {
            return Ok(false);
        }
        let value = self
            .increment(
                &reset_confirm_ip_counter_key(ip),
                self.config.password_reset_confirm_window,
            )
            .await?;
        if value > self.config.password_reset_confirm_max_per_ip {
            self.set_lock(
                &reset_confirm_ip_lock_key(ip),
                self.config.password_reset_confirm_window,
            )
            .await?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn exists(&self, key: &str) -> Result<bool, RateLimitError> {
        match self.config.backend {
            RateLimitBackend::Redis => self.redis()?.exists(key).await.map_err(Into::into),
            RateLimitBackend::Memory => Ok(self.memory.exists(key).await),
        }
    }

    async fn increment(&self, key: &str, ttl: Duration) -> Result<u64, RateLimitError> {
        match self.config.backend {
            RateLimitBackend::Redis => self
                .redis()?
                .increment_with_ttl(key, ttl)
                .await
                .map_err(Into::into),
            RateLimitBackend::Memory => Ok(self.memory.increment(key, ttl).await),
        }
    }

    async fn set_lock(&self, key: &str, ttl: Duration) -> Result<(), RateLimitError> {
        match self.config.backend {
            RateLimitBackend::Redis => self.redis()?.set_flag(key, ttl).await.map_err(Into::into),
            RateLimitBackend::Memory => {
                self.memory.set(key, ttl).await;
                Ok(())
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<(), RateLimitError> {
        match self.config.backend {
            RateLimitBackend::Redis => self.redis()?.delete(key).await.map_err(Into::into),
            RateLimitBackend::Memory => {
                self.memory.delete(key).await;
                Ok(())
            }
        }
    }

    fn redis(&self) -> Result<&RedisCache, RateLimitError> {
        self.redis
            .as_ref()
            .filter(|redis| redis.is_enabled())
            .ok_or(RateLimitError::BackendUnavailable)
    }
}

impl MemoryRateLimitStore {
    async fn exists(&self, key: &str) -> bool {
        self.prune_and_get(key).await.is_some()
    }

    async fn increment(&self, key: &str, ttl: Duration) -> u64 {
        let mut entries = self.entries.lock().await;
        prune_expired(&mut entries);
        let now = Instant::now();
        let entry = entries.entry(key.to_string()).or_insert(MemoryEntry {
            value: 0,
            expires_at: now + ttl,
        });
        entry.value += 1;
        entry.value
    }

    async fn set(&self, key: &str, ttl: Duration) {
        let mut entries = self.entries.lock().await;
        prune_expired(&mut entries);
        entries.insert(
            key.to_string(),
            MemoryEntry {
                value: 1,
                expires_at: Instant::now() + ttl,
            },
        );
    }

    async fn delete(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }

    async fn prune_and_get(&self, key: &str) -> Option<u64> {
        let mut entries = self.entries.lock().await;
        prune_expired(&mut entries);
        entries.get(key).map(|entry| entry.value)
    }
}

fn prune_expired(entries: &mut HashMap<String, MemoryEntry>) {
    let now = Instant::now();
    entries.retain(|_, entry| entry.expires_at > now);
}

pub fn normalize_key_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn ip_key(ip: Option<&str>) -> &str {
    ip.filter(|value| !value.is_empty()).unwrap_or("unknown")
}

fn login_user_counter_key(name: &str) -> String {
    format!("rl:login:user:{}", normalize_key_value(name))
}

fn login_user_lock_key(name: &str) -> String {
    format!("lock:login:user:{}", normalize_key_value(name))
}

fn login_ip_counter_key(ip: Option<&str>) -> String {
    format!("rl:login:ip:{}", ip_key(ip))
}

fn login_ip_lock_key(ip: Option<&str>) -> String {
    format!("lock:login:ip:{}", ip_key(ip))
}

fn reset_email_counter_key(email: &str) -> String {
    format!("rl:reset:email:{}", normalize_key_value(email))
}

fn reset_email_lock_key(email: &str) -> String {
    format!("lock:reset:email:{}", normalize_key_value(email))
}

fn reset_ip_counter_key(ip: Option<&str>) -> String {
    format!("rl:reset:ip:{}", ip_key(ip))
}

fn reset_ip_lock_key(ip: Option<&str>) -> String {
    format!("lock:reset:ip:{}", ip_key(ip))
}

fn reset_confirm_ip_counter_key(ip: Option<&str>) -> String {
    format!("rl:reset_confirm:ip:{}", ip_key(ip))
}

fn reset_confirm_ip_lock_key(ip: Option<&str>) -> String {
    format!("lock:reset_confirm:ip:{}", ip_key(ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_config() -> RateLimitConfig {
        RateLimitConfig {
            enabled: true,
            backend: RateLimitBackend::Memory,
            login_fail_window: Duration::from_secs(60),
            login_fail_max_per_user: 2,
            login_fail_max_per_ip: 10,
            login_fail_lock: Duration::from_secs(60),
            password_reset_window: Duration::from_secs(60),
            password_reset_max_per_email: 2,
            password_reset_max_per_ip: 10,
            password_reset_confirm_window: Duration::from_secs(60),
            password_reset_confirm_max_per_ip: 2,
        }
    }

    #[tokio::test]
    async fn memory_login_limit_blocks_and_clears_user_bucket() {
        let limiter = RateLimiter::new(memory_config(), None);

        assert!(
            !limiter
                .login_is_blocked("Alice", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            !limiter
                .record_login_failure("Alice", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            !limiter
                .record_login_failure("alice", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            limiter
                .record_login_failure("ALICE", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            limiter
                .login_is_blocked("alice", Some("127.0.0.1"))
                .await
                .unwrap()
        );

        limiter.clear_login_user("alice").await.unwrap();

        assert!(
            !limiter
                .login_is_blocked("alice", Some("127.0.0.1"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn memory_reset_request_limit_blocks_generically() {
        let limiter = RateLimiter::new(memory_config(), None);

        assert!(
            !limiter
                .password_reset_request_is_blocked("user@example.test", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            !limiter
                .password_reset_request_is_blocked("USER@example.test", Some("127.0.0.1"))
                .await
                .unwrap()
        );
        assert!(
            limiter
                .password_reset_request_is_blocked(" user@example.test ", Some("127.0.0.1"))
                .await
                .unwrap()
        );
    }
}
