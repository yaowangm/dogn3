mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
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
async fn board_endpoint_returns_board_metadata_and_tree_posts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/11?page=1&page_size=1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["board"]["id"], 11);
    assert_eq!(body["board"]["name"], "Chat");
    assert_eq!(body["board"]["category_name"], "General");
    assert_eq!(
        body["board"]["master_users"],
        serde_json::json!([{"id": 2, "name": "Bob"}, {"id": 3, "name": "Carol"}])
    );
    assert_eq!(body["pager"]["page"], 1);
    assert_eq!(body["pager"]["page_size"], 1);
    assert_eq!(body["pager"]["total_pages"], 4);
    assert_eq!(body["pager"]["has_next"], true);
    assert!(body["recent_announcement_post"].is_null());
    assert_eq!(body["boards"].as_array().expect("boards").len(), 3);

    let trees = body["trees"].as_array().expect("trees should be an array");
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["root_id"], 106);
    assert_eq!(trees[0]["posts"][0]["subject"], "Second chat root");
    assert_eq!(trees[0]["posts"][0]["level"], 0);
    assert_eq!(trees[0]["posts"][0]["size"], 356);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_endpoint_returns_recent_announcement_post_when_present() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/10?page=1&page_size=1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["recent_announcement_post"]["id"], 100);
    assert_eq!(body["recent_announcement_post"]["subject"], "Announcement");
    assert_eq!(body["recent_announcement_post"]["post_type"], 3);
    assert_eq!(
        body["recent_announcement_post"]["post_time"],
        "2024-02-01 09:00"
    );
    assert_eq!(body["recent_announcement_post"]["user_name"], "Alice");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_endpoint_orders_posts_inside_tree_by_tree_order() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/11?page=1&page_size=4").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pager"]["page"], 1);

    let posts = body["trees"][1]["posts"]
        .as_array()
        .expect("posts should be an array");
    let subjects = posts
        .iter()
        .map(|post| post["subject"].as_str().expect("subject"))
        .collect::<Vec<_>>();
    let levels = posts
        .iter()
        .map(|post| post["level"].as_i64().expect("level"))
        .collect::<Vec<_>>();

    assert_eq!(
        subjects,
        vec!["Original root", "Original reply", "Nested reply"]
    );
    assert_eq!(levels, vec![0, 1, 2]);
    assert_eq!(posts[0]["reply_time"], "2024-02-02 09:10");
    assert_eq!(posts[0]["link_url"], "https://example.test/reference");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_endpoint_uses_configured_default_page_size() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/11").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pager"]["page_size"], 50);
    assert_eq!(body["pager"]["total_pages"], 1);
    let post_count = body["trees"]
        .as_array()
        .expect("trees")
        .iter()
        .map(|tree| tree["posts"].as_array().expect("posts").len())
        .sum::<usize>();
    assert_eq!(post_count, 4);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_endpoint_returns_not_found_for_missing_board() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/999999").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_endpoint_counts_only_visible_posts_for_pager() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool);

    let (status, body) = get_json(public_app, "/api/boards/20?page=1&page_size=1").await;
    let (_, authenticated) = get_json_with_cookie(
        authenticated_app,
        "/api/boards/20?page=1&page_size=1",
        Some(&cookie),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["board"]["post_count"], 18);
    assert_eq!(body["pager"]["total_posts"], 1);
    assert_eq!(body["pager"]["total_pages"], 1);
    assert_eq!(body["pager"]["has_next"], false);
    assert_eq!(body["trees"][0]["posts"][0]["state"], 1);
    assert_eq!(body["trees"][0]["posts"][0]["has_link"], true);
    assert!(body["trees"][0]["posts"][0]["link_url"].is_null());
    assert_eq!(
        authenticated["trees"][0]["posts"][0]["link_url"],
        "https://example.test/private"
    );
}
