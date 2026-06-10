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
async fn post_endpoint_returns_detail_resources_points_and_tree() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original_last_update: Option<String> = sqlx::query_scalar(
        "SELECT to_char(last_update_time, 'YYYY-MM-DD HH24:MI:SS.US') FROM post WHERE id = 101",
    )
    .fetch_one(&pool)
    .await
    .expect("post fixture should be readable");
    sqlx::query("UPDATE post SET last_update_time = '2024-02-02 10:30:00', content_format = 1 WHERE id = 101")
        .execute(&pool)
        .await
        .expect("post update time and format should be adjustable");
    sqlx::query("UPDATE post SET content_format = 1 WHERE id = 100")
        .execute(&pool)
        .await
        .expect("signature format should be adjustable");
    let app = common::test_app(pool.clone());

    let (status, body) = get_json(app, "/api/posts/101").await;

    sqlx::query(
        "UPDATE post SET last_update_time = $1::timestamp, content_format = 0 WHERE id = 101",
    )
    .bind(original_last_update)
    .execute(&pool)
    .await
    .expect("post fixture should be restored");
    sqlx::query("UPDATE post SET content_format = 0 WHERE id = 100")
        .execute(&pool)
        .await
        .expect("signature fixture should be restored");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["board"]["id"], 11);
    assert_eq!(body["board"]["name"], "Chat");
    assert_eq!(body["reply_open"], false);
    assert_eq!(body["can_reply"], false);
    assert_eq!(body["can_delete"], false);
    assert_eq!(body["can_favorite"], false);
    assert_eq!(body["is_favorite"], false);
    assert_eq!(body["post"]["subject"], "Original root");
    assert_eq!(
        body["post"]["content"],
        "A full original post.\nSecond paragraph."
    );
    assert_eq!(body["post"]["content_format"], 1);
    assert_eq!(body["post"]["has_content"], true);
    assert_eq!(body["post"]["last_update_time"], "2024-02-02 10:30");
    assert_eq!(body["post"]["link_url"], "https://example.test/reference");
    assert_eq!(
        body["post"]["signature"]["content"],
        "Signature: keep learning."
    );
    assert_eq!(body["post"]["signature"]["content_format"], 1);
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
async fn post_access_count_updates_once_per_login_session_on_detail_page_only() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    sqlx::query(
        r#"
        INSERT INTO post (
            id, subject, board_id, user_id, user_name, post_time, reply_time, size,
            reply_count, access_count, point, type, state, content, parent_id, root_id,
            level, order_num
        )
        VALUES (
            901, 'Access count fixture', 11, 2, 'Bob', '2024-02-06 09:00:00',
            '2024-02-06 09:00:00', 24, 0, 7, 0, 0, 0, 'Access count body',
            0, 901, 0, 0
        )
        ON CONFLICT (id) DO UPDATE
        SET access_count = 7,
            state = 0
        "#,
    )
    .execute(&pool)
    .await
    .expect("access-count fixture should be written");

    let public_app = common::test_app(pool.clone());
    let (first_app, first_cookie) = common::authenticated_test_app(pool.clone());
    let (second_app, second_cookie) = common::authenticated_test_app(pool.clone());

    let (public_status, public_body) = get_json(public_app.clone(), "/api/posts/901").await;
    let public_count: i32 = sqlx::query_scalar("SELECT access_count FROM post WHERE id = 901")
        .fetch_one(&pool)
        .await
        .expect("access count should be readable");
    let (first_status, first_body) =
        get_json_with_cookie(first_app.clone(), "/api/posts/901", Some(&first_cookie)).await;
    let count_after_first: i32 = sqlx::query_scalar("SELECT access_count FROM post WHERE id = 901")
        .fetch_one(&pool)
        .await
        .expect("access count should be readable");
    let (repeat_status, repeat_body) =
        get_json_with_cookie(first_app, "/api/posts/901", Some(&first_cookie)).await;
    let count_after_repeat: i32 =
        sqlx::query_scalar("SELECT access_count FROM post WHERE id = 901")
            .fetch_one(&pool)
            .await
            .expect("access count should be readable");
    let (list_status, _) = get_json_with_cookie(
        second_app.clone(),
        "/api/post_lists/901",
        Some(&second_cookie),
    )
    .await;
    let (print_status, _) = get_json_with_cookie(
        second_app.clone(),
        "/api/post_prints/901",
        Some(&second_cookie),
    )
    .await;
    let count_after_list_print: i32 =
        sqlx::query_scalar("SELECT access_count FROM post WHERE id = 901")
            .fetch_one(&pool)
            .await
            .expect("access count should be readable");
    let (second_status, second_body) =
        get_json_with_cookie(second_app, "/api/posts/901", Some(&second_cookie)).await;
    let count_after_second: i32 =
        sqlx::query_scalar("SELECT access_count FROM post WHERE id = 901")
            .fetch_one(&pool)
            .await
            .expect("access count should be readable");

    sqlx::query("DELETE FROM post WHERE id = 901")
        .execute(&pool)
        .await
        .expect("access-count fixture should be removed");

    assert_eq!(public_status, StatusCode::OK);
    assert_eq!(public_body["post"]["access_count"], 7);
    assert_eq!(public_count, 7);
    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first_body["post"]["access_count"], 8);
    assert_eq!(count_after_first, 8);
    assert_eq!(repeat_status, StatusCode::OK);
    assert_eq!(repeat_body["post"]["access_count"], 8);
    assert_eq!(count_after_repeat, 8);
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(print_status, StatusCode::OK);
    assert_eq!(count_after_list_print, 8);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(second_body["post"]["access_count"], 9);
    assert_eq!(count_after_second, 9);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn encrypted_signature_is_visible_only_to_authenticated_viewers() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool.clone());
    let signature_state_before: i32 = sqlx::query_scalar("SELECT state FROM post WHERE id = 100")
        .fetch_one(&pool)
        .await
        .expect("signature fixture should be readable");
    sqlx::query("UPDATE post SET state = 1 WHERE id = 100")
        .execute(&pool)
        .await
        .expect("signature fixture should become encrypted");

    let (public_status, public_body) = get_json(public_app, "/api/posts/101").await;
    let (authenticated_status, authenticated_body) =
        get_json_with_cookie(authenticated_app, "/api/posts/101", Some(&cookie)).await;

    sqlx::query("UPDATE post SET state = $1 WHERE id = 100")
        .bind(signature_state_before)
        .execute(&pool)
        .await
        .expect("signature fixture should be restored");

    assert_eq!(public_status, StatusCode::OK);
    assert!(public_body["post"]["signature"].is_null());
    assert_eq!(authenticated_status, StatusCode::OK);
    assert_eq!(
        authenticated_body["post"]["signature"]["content"],
        "Signature: keep learning."
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_endpoint_exposes_delete_only_to_board_master_or_administrator() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (master_app, master_cookie) = common::authenticated_test_app(pool.clone());
    let (admin_app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );
    let (owner_app, owner_cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 5,
        },
    );

    let (_, managed_board_post) =
        get_json_with_cookie(master_app.clone(), "/api/posts/101", Some(&master_cookie)).await;
    let (_, other_board_post) =
        get_json_with_cookie(master_app, "/api/posts/103", Some(&master_cookie)).await;
    let (_, admin_post) =
        get_json_with_cookie(admin_app, "/api/posts/103", Some(&admin_cookie)).await;
    let (_, owner_leaf_root) =
        get_json_with_cookie(owner_app, "/api/posts/103", Some(&owner_cookie)).await;

    assert_eq!(managed_board_post["can_delete"], true);
    assert_eq!(managed_board_post["delete_post_count"], 3);
    assert_eq!(managed_board_post["can_favorite"], true);
    assert_eq!(managed_board_post["is_favorite"], true);
    assert_eq!(other_board_post["can_delete"], false);
    assert_eq!(admin_post["can_delete"], true);
    assert_eq!(owner_leaf_root["can_delete"], true);
    assert_eq!(owner_leaf_root["delete_post_count"], 1);
    assert_eq!(owner_leaf_root["can_favorite"], true);
    assert_eq!(owner_leaf_root["is_favorite"], false);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn encrypted_post_redacts_content_until_login_and_hides_deleted_posts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool);

    let (encrypted_status, encrypted) = get_json(public_app.clone(), "/api/posts/103").await;
    let (list_status, list) = get_json(public_app.clone(), "/api/post_lists/103").await;
    let (print_status, print) = get_json(public_app.clone(), "/api/post_prints/103").await;
    let (visible_status, visible) =
        get_json_with_cookie(authenticated_app, "/api/posts/103", Some(&cookie)).await;
    let (deleted_status, _) = get_json(public_app.clone(), "/api/posts/104").await;
    let (unknown_status, _) = get_json(public_app.clone(), "/api/posts/107").await;
    let (missing_status, _) = get_json(public_app, "/api/posts/999999").await;

    assert_eq!(encrypted_status, StatusCode::OK);
    assert_eq!(encrypted["post"]["state"], 1);
    assert_eq!(encrypted["post"]["content_visible"], false);
    assert_eq!(encrypted["post"]["has_content"], false);
    assert_eq!(encrypted["post"]["has_link"], true);
    assert_eq!(encrypted["post"]["has_image"], true);
    assert!(encrypted["post"]["content"].is_null());
    assert!(encrypted["post"]["link_url"].is_null());
    assert!(encrypted["post"]["image_url"].is_null());
    assert!(encrypted["post"]["signature"].is_null());
    assert_eq!(
        encrypted["post"]["point_awards"].as_array().unwrap().len(),
        0
    );
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(list["posts"][0]["content_visible"], false);
    assert_eq!(list["posts"][0]["has_content"], false);
    assert!(list["posts"][0]["content"].is_null());
    assert_eq!(print_status, StatusCode::OK);
    assert_eq!(print["post"]["content_visible"], false);
    assert!(print["post"]["content"].is_null());
    assert_eq!(visible_status, StatusCode::OK);
    assert_eq!(visible["reply_open"], false);
    assert_eq!(visible["can_reply"], false);
    assert_eq!(visible["post"]["content_visible"], true);
    assert_eq!(visible["post"]["has_content"], true);
    assert_eq!(visible["post"]["content"], "Encrypted body.");
    assert_eq!(visible["post"]["link_url"], "https://example.test/private");
    assert_eq!(visible["post"]["image_url"], "private.JPG");
    assert_eq!(
        visible["post"]["signature"]["content"],
        "Signature: keep learning."
    );
    assert_eq!(visible["post"]["point_awards"][0]["user_name"], "Carol");
    assert_eq!(deleted_status, StatusCode::NOT_FOUND);
    assert_eq!(unknown_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_in_recent_tree_exposes_reply_action_only_to_logged_in_viewer() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original_post_time: Option<String> = sqlx::query_scalar(
        "SELECT to_char(post_time, 'YYYY-MM-DD HH24:MI:SS.US') FROM post WHERE id = 106",
    )
    .fetch_one(&pool)
    .await
    .expect("post fixture should be readable");
    sqlx::query("UPDATE post SET post_time = CURRENT_TIMESTAMP WHERE id = 106")
        .execute(&pool)
        .await
        .expect("post time should update");
    let public_app = common::test_app(pool.clone());
    let (authenticated_app, cookie) = common::authenticated_test_app(pool.clone());

    let (_, public) = get_json(public_app, "/api/posts/106").await;
    let (_, authenticated) =
        get_json_with_cookie(authenticated_app, "/api/posts/106", Some(&cookie)).await;

    sqlx::query("UPDATE post SET post_time = $1::timestamp WHERE id = 106")
        .bind(original_post_time)
        .execute(&pool)
        .await
        .expect("post time fixture should be restored");

    assert_eq!(public["can_reply"], false);
    assert_eq!(public["reply_open"], true);
    assert_eq!(authenticated["reply_open"], true);
    assert_eq!(authenticated["can_reply"], true);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn frozen_session_cannot_read_encrypted_post_content() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool.clone());
    sqlx::query("UPDATE user_info SET level = 0 WHERE id = 2")
        .execute(&pool)
        .await
        .expect("member fixture should be frozen");

    let (status, body) = get_json_with_cookie(app, "/api/posts/103", Some(&cookie)).await;

    sqlx::query("UPDATE user_info SET level = 5 WHERE id = 2")
        .execute(&pool)
        .await
        .expect("board-master fixture should be restored");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["post"]["content_visible"], false);
    assert!(body["post"]["content"].is_null());
    assert!(body["post"]["link_url"].is_null());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_list_endpoint_returns_full_tree_oldest_first() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/post_lists/102").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["selected_post_id"], 102);
    assert_eq!(body["board"]["name"], "Chat");

    let posts = body["posts"].as_array().expect("posts should be an array");
    let ids = posts
        .iter()
        .map(|post| post["id"].as_i64().expect("post id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![101, 102, 105]);
    assert_eq!(
        posts[0]["content"],
        "A full original post.\nSecond paragraph."
    );
    assert_eq!(posts[0]["point_awards"][0]["user_name"], "Bob");
    assert_eq!(posts[1]["level"], 1);
    assert_eq!(posts[1]["has_content"], false);
    assert!(posts[1]["content"].is_null());
    assert_eq!(posts[0]["order_num"], 0);
    assert_eq!(posts[2]["order_num"], 2);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_print_endpoint_returns_only_printable_post_context() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json(app, "/api/post_prints/101").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["board"]["name"], "Chat");
    assert_eq!(body["post"]["subject"], "Original root");
    assert_eq!(
        body["post"]["signature"]["content"],
        "Signature: keep learning."
    );
    assert!(body.get("tree").is_none());
    assert!(body.get("boards").is_none());
}
