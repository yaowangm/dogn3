mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
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
}
