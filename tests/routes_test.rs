mod common;

use std::{fs, time::SystemTime};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dogn3::{build_router, state::AppState};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

#[tokio::test]
async fn configured_image_directory_serves_post_images() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory =
        std::env::temp_dir().join(format!("dogn3-test-images-{}-{unique}", std::process::id()));
    let image_path = image_directory.join("200809/sample.JPG");
    fs::create_dir_all(image_path.parent().expect("image parent")).expect("create image fixture");
    fs::write(&image_path, b"test-image").expect("write image fixture");

    let pool = PgPoolOptions::new()
        .connect_lazy("postgres:///dogn_test")
        .expect("valid lazy PostgreSQL pool");
    let app = build_router(
        AppState::new(pool, None, "Test Forum".to_string(), 50),
        &image_directory,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/200809/sample.JPG")
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
        .expect("image body should be readable")
        .to_bytes();
    assert_eq!(body.as_ref(), b"test-image");

    let legacy_response = app
        .oneshot(
            Request::builder()
                .uri("/images/pic/200809/sample.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("legacy image route should respond");
    assert_eq!(legacy_response.status(), StatusCode::OK);

    fs::remove_dir_all(image_directory).expect("clean image fixture");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn index_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
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
    let body = String::from_utf8(body.to_vec()).expect("body should be utf-8");

    assert!(body.contains("<!doctype html>"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/board/11")
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
    let body = String::from_utf8(body.to_vec()).expect("body should be utf-8");

    assert!(body.contains("<dogn-app-shell>"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post/101")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn health_endpoint_reports_database_ok() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
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
    let body: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["cache"], "disabled");
}
