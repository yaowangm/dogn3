use std::time::Duration;

use dogn3::{
    auth::AuthenticatedUser,
    build_router,
    cache::RedisCache,
    state::{AppState, AuthRuntimeConfig},
};
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
    build_router(test_state(pool))
}

#[allow(dead_code)]
pub fn authenticated_test_app(pool: PgPool) -> (axum::Router, String) {
    authenticated_app(test_state(pool))
}

#[allow(dead_code)]
pub fn authenticated_test_app_as(pool: PgPool, user: AuthenticatedUser) -> (axum::Router, String) {
    let state = test_state(pool);
    let token = state.sessions.create(user);
    (build_router(state), format!("dogn_session={token}"))
}

#[allow(dead_code)]
pub fn authenticated_test_app_with_cache(
    pool: PgPool,
    cache: RedisCache,
) -> (axum::Router, String) {
    authenticated_app(AppState::new(
        pool,
        Some(cache),
        "Test Forum".to_string(),
        50,
        10,
        std::env::temp_dir().join("dogn3-test-images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    ))
}

fn authenticated_app(state: AppState) -> (axum::Router, String) {
    let token = state.sessions.create(AuthenticatedUser {
        id: 2,
        name: "Bob".to_string(),
        level: 1,
    });
    (build_router(state), format!("dogn_session={token}"))
}

fn test_state(pool: PgPool) -> AppState {
    AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        10,
        std::env::temp_dir().join("dogn3-test-images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    )
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
        10,
        std::env::temp_dir().join("dogn3-test-images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    ))
}
