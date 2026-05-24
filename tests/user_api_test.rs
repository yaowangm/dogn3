mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::auth::AuthenticatedUser;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    get_json_with_cookie(app, uri, None).await
}

async fn get_json_with_cookie(
    app: axum::Router,
    uri: &str,
    cookie: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().uri(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(request.body(Body::empty()).expect("valid request"))
        .await
        .expect("route should respond");
    let status = response.status();
    if status == StatusCode::OK {
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
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
async fn user_endpoint_returns_profile_and_original_activity_by_default() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/users/2").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["user"]["name"], "Bob");
    assert_eq!(body["user"]["intro"], "Rust reader.");
    assert_eq!(body["user"]["point"], 90);
    assert_eq!(body["user"]["doc_count"], 3);
    assert_eq!(body["user"]["last_login"], "2024-02-08 09:30");
    assert_eq!(
        body["latest_signature"]["content"],
        "A full original post.\nSecond paragraph."
    );
    assert!(body.get("private_details").is_none());
    assert_eq!(body["can_update"], false);
    assert_eq!(body["activity"], "original");
    assert_eq!(body["pager"]["page_size"], 50);
    assert_eq!(body["pager"]["total_posts"], 1);
    assert_eq!(body["posts"][0]["id"], 101);
    assert_eq!(body["boards"].as_array().expect("boards").len(), 3);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_endpoint_allows_profile_operations_only_for_owner_or_admin() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (owner_app, owner_cookie) = common::authenticated_test_app(pool.clone());
    let (admin_app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );
    let (other_app, other_cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 1,
        },
    );

    let (_, owner) = get_json_with_cookie(owner_app, "/api/users/2", Some(&owner_cookie)).await;
    let (_, admin) = get_json_with_cookie(admin_app, "/api/users/2", Some(&admin_cookie)).await;
    let (_, other) = get_json_with_cookie(other_app, "/api/users/2", Some(&other_cookie)).await;

    assert_eq!(owner["can_update"], true);
    assert_eq!(owner["private_details"]["last_login_ip"], "192.0.2.2");
    assert_eq!(owner["private_details"]["intro_user_id"], 1);
    assert_eq!(owner["private_details"]["intro_user_name"], "Alice");
    assert_eq!(owner["private_details"]["login_count"], 21);
    assert_eq!(admin["can_update"], true);
    assert_eq!(admin["private_details"]["last_login_ip"], "192.0.2.2");
    assert_eq!(admin["private_details"]["intro_user_id"], 1);
    assert_eq!(other["can_update"], false);
    assert!(other.get("private_details").is_none());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_endpoint_pages_favorites_and_redacts_encrypted_resources_until_login() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool);

    let (status, body) = get_json(
        public_app,
        "/api/users/2?activity=favorites&page=1&page_size=1",
    )
    .await;
    let (_, authenticated) = get_json_with_cookie(
        authenticated_app,
        "/api/users/2?activity=favorites&page=1&page_size=1",
        Some(&cookie),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["activity"], "favorites");
    assert_eq!(body["pager"]["total_posts"], 2);
    assert_eq!(body["pager"]["total_pages"], 2);
    assert_eq!(body["pager"]["has_next"], true);
    assert_eq!(body["posts"][0]["id"], 103);
    assert_eq!(body["posts"][0]["has_link"], true);
    assert_eq!(body["posts"][0]["has_image"], true);
    assert!(body["posts"][0]["link_url"].is_null());
    assert!(body["posts"][0]["image_url"].is_null());
    assert_eq!(
        authenticated["posts"][0]["link_url"],
        "https://example.test/private"
    );
    assert_eq!(authenticated["posts"][0]["image_url"], "pic/private.JPG");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_endpoint_lists_signature_posts_by_post_id_descending() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/users/2?activity=signatures").await;

    assert_eq!(status, StatusCode::OK);
    let ids = body["posts"]
        .as_array()
        .expect("posts")
        .iter()
        .map(|post| post["id"].as_i64().expect("post id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![101, 100]);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_endpoint_returns_not_found_for_missing_user() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/users/999999").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}
