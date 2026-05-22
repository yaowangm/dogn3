use std::time::Duration;

use dogn3::{build_router, cache::RedisCache, state::AppState};
use sqlx::PgPool;

pub async fn test_pool() -> Option<PgPool> {
    let database_url = match std::env::var("TEST_DATABASE_URL") {
        Ok(database_url) => database_url,
        Err(_) => return None,
    };

    Some(
        PgPool::connect(&database_url)
            .await
            .expect("failed to connect to test database"),
    )
}

pub fn test_app(pool: PgPool) -> axum::Router {
    build_router(AppState::new(pool, None, "Test Forum".to_string(), 50))
}

#[allow(dead_code)]
pub async fn test_cache() -> Option<RedisCache> {
    let redis_url = match std::env::var("TEST_REDIS_URL") {
        Ok(redis_url) => redis_url,
        Err(_) => return None,
    };
    let key_prefix = format!("dogn3:test:{}", std::process::id());
    let cache = RedisCache::new(&redis_url, key_prefix, Duration::from_secs(60))
        .expect("failed to create test Redis cache");
    cache.ping().await.expect("failed to ping test Redis");
    Some(cache)
}

#[allow(dead_code)]
pub fn test_app_with_cache(pool: PgPool, cache: RedisCache) -> axum::Router {
    build_router(AppState::new(
        pool,
        Some(cache),
        "Test Forum".to_string(),
        50,
    ))
}
