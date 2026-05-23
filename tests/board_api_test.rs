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
        body["board"]["master_names"],
        serde_json::json!(["Bob", "Carol"])
    );
    assert_eq!(body["pager"]["page"], 1);
    assert_eq!(body["pager"]["page_size"], 1);
    assert_eq!(body["pager"]["total_pages"], 4);
    assert_eq!(body["pager"]["has_next"], true);
    assert_eq!(body["boards"].as_array().expect("boards").len(), 3);

    let trees = body["trees"].as_array().expect("trees should be an array");
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["root_id"], 106);
    assert_eq!(trees[0]["posts"][0]["subject"], "Second chat root");
    assert_eq!(trees[0]["posts"][0]["level"], 0);
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
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/boards/20?page=1&page_size=1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["board"]["post_count"], 18);
    assert_eq!(body["pager"]["total_posts"], 1);
    assert_eq!(body["pager"]["total_pages"], 1);
    assert_eq!(body["pager"]["has_next"], false);
}
