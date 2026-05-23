mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&body).expect("response should be json");
    (status, body)
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_endpoint_returns_detail_resources_points_and_tree() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/posts/101").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["board"]["id"], 11);
    assert_eq!(body["board"]["name"], "Chat");
    assert_eq!(body["post"]["subject"], "Original root");
    assert_eq!(
        body["post"]["content"],
        "A full original post.\nSecond paragraph."
    );
    assert_eq!(body["post"]["link_url"], "https://example.test/reference");
    assert_eq!(
        body["post"]["signature"]["content"],
        "Signature: keep learning."
    );
    assert_eq!(body["post"]["point_awards"][0]["user_name"], "Bob");
    assert_eq!(body["post"]["point_awards"][0]["point"], 8);
    assert_eq!(body["post"]["point_awards"][1]["user_name"], "Carol");
    assert_eq!(
        body["tree"]["posts"].as_array().expect("tree posts").len(),
        3
    );
    assert_eq!(
        body["tree"]["posts"][0]["link_url"],
        "https://example.test/reference"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_endpoint_shows_encrypted_posts_but_hides_deleted_and_missing_posts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (encrypted_status, encrypted) = get_json(app.clone(), "/api/posts/103").await;
    let (deleted_status, _) = get_json(app.clone(), "/api/posts/104").await;
    let (missing_status, _) = get_json(app, "/api/posts/999999").await;

    assert_eq!(encrypted_status, StatusCode::OK);
    assert_eq!(encrypted["post"]["state"], 1);
    assert_eq!(deleted_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
}
