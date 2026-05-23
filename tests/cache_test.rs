mod common;

use std::time::{Duration, Instant};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const HOME_CACHE_KEY: &str = "api:home:v1";

async fn get_home(app: axum::Router) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();

    serde_json::from_slice(&body).expect("response should be json")
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and TEST_REDIS_URL; use ./scripts/test.sh"]
async fn home_endpoint_uses_cached_response_until_cache_is_deleted() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let Some(cache) = common::test_cache().await else {
        return;
    };
    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");

    let app = common::test_app_with_cache(pool.clone(), cache.clone());
    let first = get_home(app.clone()).await;
    assert_eq!(first["recent_root_posts"][0]["subject"], "Second chat root");

    sqlx::query("UPDATE post SET subject = 'Changed second chat root' WHERE id = 106")
        .execute(&pool)
        .await
        .expect("fixture update should succeed");

    let cached = get_home(app.clone()).await;
    assert_eq!(
        cached["recent_root_posts"][0]["subject"],
        "Second chat root"
    );

    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");
    let refreshed = get_home(app).await;
    assert_eq!(
        refreshed["recent_root_posts"][0]["subject"],
        "Changed second chat root"
    );

    sqlx::query("UPDATE post SET subject = 'Second chat root' WHERE id = 106")
        .execute(&pool)
        .await
        .expect("fixture restore should succeed");
    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and TEST_REDIS_URL; use ./scripts/test.sh"]
async fn home_endpoint_returns_same_response_with_or_without_cache() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let Some(cache) = common::test_cache().await else {
        return;
    };
    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");

    let without_cache = get_home(common::test_app(pool.clone())).await;
    let with_cache = get_home(common::test_app_with_cache(pool, cache.clone())).await;

    assert_eq!(with_cache, without_cache);

    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and TEST_REDIS_URL; use ./scripts/test.sh"]
async fn cached_home_endpoint_is_faster_than_uncached_database_path() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let Some(cache) = common::test_cache().await else {
        return;
    };
    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");

    let cached_app = common::test_app_with_cache(pool.clone(), cache.clone());
    let uncached_app = common::test_app(pool);

    let _ = get_home(cached_app.clone()).await;

    let uncached_duration = time_requests(uncached_app, 10).await;
    let cached_duration = time_requests(cached_app, 10).await;

    cache.delete(HOME_CACHE_KEY).await.expect("cache cleanup");

    assert!(
        cached_duration < uncached_duration,
        "cached duration {cached_duration:?} should be faster than uncached duration {uncached_duration:?}"
    );
}

async fn time_requests(app: axum::Router, count: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..count {
        let _ = get_home(app.clone()).await;
    }
    start.elapsed()
}
