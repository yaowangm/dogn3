mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn home_endpoint_returns_default_page_sections() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool);

    let response = public_app
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(
        body["recent_announcement_posts"].as_array().unwrap().len(),
        1
    );
    assert_eq!(body["recent_root_posts"].as_array().unwrap().len(), 4);
    assert_eq!(body["recent_original_posts"].as_array().unwrap().len(), 1);
    assert_eq!(body["recent_forward_posts"].as_array().unwrap().len(), 1);
    assert_eq!(body["new_users"].as_array().unwrap().len(), 3);
    assert_eq!(body["top_point_users"].as_array().unwrap().len(), 3);
    assert_eq!(body["boards"].as_array().unwrap().len(), 3);

    assert_eq!(
        body["recent_announcement_posts"][0]["subject"],
        "Announcement"
    );
    assert_eq!(body["recent_root_posts"][0]["subject"], "Second chat root");
    assert_eq!(body["recent_original_posts"][0]["subject"], "Original root");
    assert_eq!(
        body["recent_original_posts"][0]["link_url"],
        "https://example.test/reference"
    );
    assert_eq!(body["recent_forward_posts"][0]["subject"], "Forward root");
    assert_eq!(body["recent_forward_posts"][0]["has_link"], true);
    assert_eq!(body["recent_forward_posts"][0]["has_image"], true);
    assert!(body["recent_forward_posts"][0]["link_url"].is_null());
    assert!(body["recent_forward_posts"][0]["image_url"].is_null());
    assert_eq!(body["new_users"][0]["name"], "Carol");
    assert_eq!(body["top_point_users"][0]["name"], "Bob");

    let root_subjects = body["recent_root_posts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|post| post["subject"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!root_subjects.contains(&"Deleted root"));

    let authenticated_response = authenticated_app
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(
        authenticated_response.headers()[header::CACHE_CONTROL],
        "no-store"
    );
    let authenticated_body: Value = serde_json::from_slice(
        &authenticated_response
            .into_body()
            .collect()
            .await
            .expect("body should be readable")
            .to_bytes(),
    )
    .expect("response should be json");
    assert_eq!(
        authenticated_body["recent_forward_posts"][0]["link_url"],
        "https://example.test/private"
    );
    assert_eq!(
        authenticated_body["recent_forward_posts"][0]["image_url"],
        "pic/private.JPG"
    );
}
