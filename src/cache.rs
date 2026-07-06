use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use redis::{AsyncCommands, Client};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct RedisCache {
    client: Client,
    key_prefix: String,
    default_ttl: Duration,
    enabled: Arc<AtomicBool>,
}

impl RedisCache {
    pub fn new(url: &str, key_prefix: String, default_ttl: Duration) -> redis::RedisResult<Self> {
        let client = Client::open(url)?;

        Ok(Self {
            client,
            key_prefix,
            default_ttl,
            enabled: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    pub async fn ping(&self) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut connection).await?;
        Ok(())
    }

    pub async fn get_json<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let value: Option<String> = connection.get(self.cache_key(key)).await?;
        value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn set_json<T>(&self, key: &str, value: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        self.set_json_with_ttl(key, value, self.default_ttl).await
    }

    pub async fn set_json_with_ttl<T>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let value = serde_json::to_string(value)?;
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: () = connection
            .set_ex(self.cache_key(key), value, ttl.as_secs())
            .await?;
        Ok(())
    }

    pub async fn add_set_member(
        &self,
        key: &str,
        value: &str,
        ttl: Duration,
    ) -> redis::RedisResult<bool> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let key = self.cache_key(key);
        let added: u64 = connection.sadd(&key, value).await?;
        let _: () = connection.expire(&key, ttl.as_secs() as i64).await?;
        Ok(added > 0)
    }

    pub async fn set_members(&self, key: &str) -> redis::RedisResult<Vec<String>> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        connection.smembers(self.cache_key(key)).await
    }

    pub async fn remove_set_member(&self, key: &str, value: &str) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: () = connection.srem(self.cache_key(key), value).await?;
        Ok(())
    }

    pub async fn get_counter(&self, key: &str) -> redis::RedisResult<u64> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let value: Option<u64> = connection.get(self.cache_key(key)).await?;
        Ok(value.unwrap_or(0))
    }

    pub async fn increment(&self, key: &str) -> redis::RedisResult<u64> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        connection.incr(self.cache_key(key), 1_u64).await
    }

    pub async fn increment_with_ttl(&self, key: &str, ttl: Duration) -> redis::RedisResult<u64> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let key = self.cache_key(key);
        let value: u64 = connection.incr(&key, 1_u64).await?;
        if value == 1 {
            let _: () = connection.expire(&key, ttl.as_secs() as i64).await?;
        }
        Ok(value)
    }

    pub async fn set_flag(&self, key: &str, ttl: Duration) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: () = connection
            .set_ex(self.cache_key(key), "1", ttl.as_secs())
            .await?;
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> redis::RedisResult<bool> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        connection.exists(self.cache_key(key)).await
    }

    pub async fn delete(&self, key: &str) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: () = connection.del(self.cache_key(key)).await?;
        Ok(())
    }

    pub async fn expire(&self, key: &str, ttl: Duration) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: () = connection
            .expire(self.cache_key(key), ttl.as_secs() as i64)
            .await?;
        Ok(())
    }

    fn cache_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }
}
