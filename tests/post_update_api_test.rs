mod common;

use std::{
    fs,
    io::Cursor,
    time::{Duration, SystemTime},
};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::{
    auth::AuthenticatedUser,
    build_router,
    state::{AppState, AuthRuntimeConfig},
};
use http_body_util::BodyExt;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::Value;
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response should be json")
}

async fn get_with_cookie(
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
    (status, response_json(response).await)
}

async fn save_post(app: axum::Router, cookie: Option<&str>, body: &str) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/post_upd")
        .header("x-dogn-request", "fetch")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(
            request
                .body(Body::from(body.to_owned()))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

async fn delete_post(app: axum::Router, cookie: Option<&str>, post_id: i32) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/posts/{post_id}/delete"))
        .header("x-dogn-request", "fetch")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(request.body(Body::from("{}")).expect("valid request"))
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

async fn favorite_post(
    app: axum::Router,
    cookie: Option<&str>,
    post_id: i32,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/posts/{post_id}/favorite"))
        .header("x-dogn-request", "fetch")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(request.body(Body::from("{}")).expect("valid request"))
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

async fn upload_image(
    app: axum::Router,
    cookie: &str,
    post_id: i32,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/posts/{post_id}/image"))
                .header(header::COOKIE, cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_editor_requires_login_for_create_update_and_reply() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (create_get, create_body) =
        get_with_cookie(app.clone(), "/api/post_upd?board_id=11", None).await;
    let (update_get, _) = get_with_cookie(app.clone(), "/api/post_upd?post_id=101", None).await;
    let (reply_get, _) = get_with_cookie(app.clone(), "/api/post_upd?reply_to=101", None).await;
    let (save_status, _) = save_post(
        app,
        None,
        r#"{"board_id":11,"subject":"New","content":"Text","post_type":0,"state":0}"#,
    )
    .await;

    assert_eq!(create_get, StatusCode::UNAUTHORIZED);
    assert_eq!(create_body["error"]["code"], "authentication_required");
    assert_eq!(update_get, StatusCode::UNAUTHORIZED);
    assert_eq!(reply_get, StatusCode::UNAUTHORIZED);
    assert_eq!(save_status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn logged_in_user_creates_root_post_and_updates_derived_statistics() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool.clone());
    let board_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("board fixture should be readable");
    let user_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count FROM user_info WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("user fixture should be readable");

    let (editor_status, editor) =
        get_with_cookie(app.clone(), "/api/post_upd?board_id=11", Some(&cookie)).await;
    let (save_status, saved) = save_post(
        app,
        Some(&cookie),
        r#"{"board_id":11,"subject":"Created root","content":"Created body","post_type":1,"state":0}"#,
    )
    .await;
    let post_id = saved["post_id"].as_i64().expect("created post id") as i32;
    let post: (i32, i32, i32, i32, i32, i32, Option<String>) = sqlx::query_as(
        "SELECT user_id, parent_id, root_id, level, order_num, reply_count, link_url FROM post WHERE id = $1",
    )
    .bind(post_id)
    .fetch_one(&pool)
    .await
    .expect("created post should be readable");
    let board_after: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("updated board should be readable");
    let user_after: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count FROM user_info WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("updated user should be readable");

    sqlx::query("DELETE FROM post WHERE id = $1")
        .bind(post_id)
        .execute(&pool)
        .await
        .expect("temporary post should be removed");
    sqlx::query("UPDATE board SET post_count = $1, root_count = $2 WHERE id = 11")
        .bind(board_before.0)
        .bind(board_before.1)
        .execute(&pool)
        .await
        .expect("board fixture should be restored");
    sqlx::query("UPDATE user_info SET post_count = $1, doc_count = $2 WHERE id = 2")
        .bind(user_before.0)
        .bind(user_before.1)
        .execute(&pool)
        .await
        .expect("user fixture should be restored");

    assert_eq!(editor_status, StatusCode::OK);
    assert_eq!(editor["mode"], "create");
    assert_eq!(editor["board"]["name"], "Chat");
    assert_eq!(editor["post_subject_max_length"], 50);
    assert_eq!(editor["post_content_max_bytes"], 131_072);
    assert_eq!(editor["image_upload_max_bytes"], 2_097_152);
    assert_eq!(save_status, StatusCode::CREATED);
    assert_eq!(post, (2, 0, post_id, 0, 0, 1, None));
    assert_eq!(board_after, (5, Some(3)));
    assert_eq!(user_after, (2, Some(2)));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_editor_rejects_subject_and_utf8_content_over_configured_limits() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);
    let subject_body = serde_json::json!({
        "board_id": 11,
        "subject": "x".repeat(51),
        "content": "",
        "post_type": 0,
        "state": 0
    })
    .to_string();
    let content_body = serde_json::json!({
        "board_id": 11,
        "subject": "Valid subject",
        "content": "中".repeat(43_691),
        "post_type": 0,
        "state": 0
    })
    .to_string();

    let (subject_status, subject_error) =
        save_post(app.clone(), Some(&cookie), &subject_body).await;
    let (content_status, content_error) = save_post(app, Some(&cookie), &content_body).await;

    assert_eq!(subject_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(subject_error["error"]["code"], "invalid_subject");
    assert_eq!(content_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(content_error["error"]["code"], "content_too_large");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn root_post_owner_or_administrator_can_update_post() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original: (
        Option<String>,
        Option<String>,
        Option<i32>,
        i32,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i32>,
    ) =
        sqlx::query_as(
            "SELECT subject, content, type, state, link_name, link_url, image_url, size FROM post WHERE id = 101",
        )
        .fetch_one(&pool)
        .await
        .expect("post fixture should be readable");
    let user_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count FROM user_info WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("user fixture should be readable");
    let (owner_app, owner_cookie) = common::authenticated_test_app(pool.clone());
    let (other_app, other_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 5,
        },
    );
    let (admin_app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let (owner_editor, _) = get_with_cookie(
        owner_app.clone(),
        "/api/post_upd?post_id=101",
        Some(&owner_cookie),
    )
    .await;
    let (denied_editor, _) = get_with_cookie(
        other_app.clone(),
        "/api/post_upd?post_id=101",
        Some(&other_cookie),
    )
    .await;
    let (denied_save, _) = save_post(
        other_app,
        Some(&other_cookie),
        r#"{"post_id":101,"subject":"Denied","content":"Denied","post_type":0,"state":0}"#,
    )
    .await;
    let (owner_save, _) = save_post(
        owner_app,
        Some(&owner_cookie),
        r#"{"post_id":101,"subject":"Owner update","content":"Owner body","post_type":0,"state":0}"#,
    )
    .await;
    let (admin_save, _) = save_post(
        admin_app,
        Some(&admin_cookie),
        r#"{"post_id":101,"subject":"Admin update","content":"Admin body","post_type":1,"state":0}"#,
    )
    .await;
    let updated_post: (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT subject, link_name, link_url FROM post WHERE id = 101")
            .fetch_one(&pool)
            .await
            .expect("updated post should be readable");

    sqlx::query(
        "UPDATE post SET subject = $1, content = $2, type = $3, state = $4, link_name = $5, link_url = $6, image_url = $7, size = $8 WHERE id = 101",
    )
    .bind(original.0)
    .bind(original.1)
    .bind(original.2)
    .bind(original.3)
    .bind(original.4)
    .bind(original.5)
    .bind(original.6)
    .bind(original.7)
    .execute(&pool)
    .await
    .expect("post fixture should be restored");
    sqlx::query("UPDATE user_info SET post_count = $1, doc_count = $2 WHERE id = 2")
        .bind(user_before.0)
        .bind(user_before.1)
        .execute(&pool)
        .await
        .expect("user fixture should be restored");

    assert_eq!(owner_editor, StatusCode::OK);
    assert_eq!(denied_editor, StatusCode::FORBIDDEN);
    assert_eq!(denied_save, StatusCode::FORBIDDEN);
    assert_eq!(owner_save, StatusCode::OK);
    assert_eq!(admin_save, StatusCode::OK);
    assert_eq!(updated_post.0.as_deref(), Some("Admin update"));
    assert_eq!(updated_post.1.as_deref(), Some("Reference"));
    assert_eq!(
        updated_post.2.as_deref(),
        Some("https://example.test/reference")
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn only_administrator_can_update_non_root_post_and_its_type_remains_normal() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original: (
        Option<String>,
        Option<String>,
        Option<i32>,
        i32,
        Option<i32>,
    ) = sqlx::query_as("SELECT subject, content, type, state, size FROM post WHERE id = 102")
        .fetch_one(&pool)
        .await
        .expect("reply fixture should be readable");
    let user_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("author fixture should be readable");
    let (owner_app, owner_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 5,
        },
    );
    let (admin_app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let (owner_editor, _) = get_with_cookie(
        owner_app.clone(),
        "/api/post_upd?post_id=102",
        Some(&owner_cookie),
    )
    .await;
    let (owner_save, _) = save_post(
        owner_app,
        Some(&owner_cookie),
        r#"{"post_id":102,"subject":"Owner reply edit","content":"","state":0}"#,
    )
    .await;
    let (admin_editor, editor) = get_with_cookie(
        admin_app.clone(),
        "/api/post_upd?post_id=102",
        Some(&admin_cookie),
    )
    .await;
    let (admin_save, _) = save_post(
        admin_app,
        Some(&admin_cookie),
        r#"{"post_id":102,"subject":"Admin reply edit","content":"Edited","post_type":3,"state":0}"#,
    )
    .await;
    let updated: (Option<String>, Option<i32>) =
        sqlx::query_as("SELECT subject, type FROM post WHERE id = 102")
            .fetch_one(&pool)
            .await
            .expect("updated reply should be readable");

    sqlx::query("UPDATE post SET subject = $1, content = $2, type = $3, state = $4, size = $5 WHERE id = 102")
        .bind(original.0)
        .bind(original.1)
        .bind(original.2)
        .bind(original.3)
        .bind(original.4)
        .execute(&pool)
        .await
        .expect("reply fixture should be restored");
    sqlx::query("UPDATE user_info SET post_count = $1, doc_count = $2 WHERE id = 3")
        .bind(user_before.0)
        .bind(user_before.1)
        .execute(&pool)
        .await
        .expect("author fixture should be restored");

    assert_eq!(owner_editor, StatusCode::FORBIDDEN);
    assert_eq!(owner_save, StatusCode::FORBIDDEN);
    assert_eq!(admin_editor, StatusCode::OK);
    assert_eq!(editor["post"]["level"], 1);
    assert_eq!(admin_save, StatusCode::OK);
    assert_eq!(updated.0.as_deref(), Some("Admin reply edit"));
    assert_eq!(updated.1, Some(0));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn leaf_root_owner_may_delete_but_root_with_children_requires_moderation() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let state_before: i32 = sqlx::query_scalar("SELECT state FROM post WHERE id = 103")
        .fetch_one(&pool)
        .await
        .expect("post fixture should be readable");
    let board_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 20")
            .fetch_one(&pool)
            .await
            .expect("board fixture should be readable");
    let users_before: Vec<(i32, i32, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT id, post_count, doc_count, favorite_count FROM user_info ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("user fixtures should be readable");
    let (owner_app, owner_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 5,
        },
    );
    let temporary_reply: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO post (subject, board_id, user_id, user_name, state, parent_id, root_id, level, order_num)
        VALUES ('Temporary child', 20, 3, 'Carol', 0, 103, 103, 1, 1)
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("temporary child should insert");

    let (tree_status, _) = delete_post(owner_app.clone(), Some(&owner_cookie), 103).await;
    sqlx::query("DELETE FROM post WHERE id = $1")
        .bind(temporary_reply)
        .execute(&pool)
        .await
        .expect("temporary child should be removed");
    let (leaf_status, leaf_result) = delete_post(owner_app, Some(&owner_cookie), 103).await;
    let deleted_state: i32 = sqlx::query_scalar("SELECT state FROM post WHERE id = 103")
        .fetch_one(&pool)
        .await
        .expect("soft-deleted root should remain stored");

    sqlx::query("UPDATE post SET state = $1 WHERE id = 103")
        .bind(state_before)
        .execute(&pool)
        .await
        .expect("post fixture should be restored");
    sqlx::query("UPDATE board SET post_count = $1, root_count = $2 WHERE id = 20")
        .bind(board_before.0)
        .bind(board_before.1)
        .execute(&pool)
        .await
        .expect("board fixture should be restored");
    for (id, post_count, doc_count, favorite_count) in users_before {
        sqlx::query(
            "UPDATE user_info SET post_count = $1, doc_count = $2, favorite_count = $3 WHERE id = $4",
        )
        .bind(post_count)
        .bind(doc_count)
        .bind(favorite_count)
        .bind(id)
        .execute(&pool)
        .await
        .expect("user fixture should be restored");
    }

    assert_eq!(tree_status, StatusCode::FORBIDDEN);
    assert_eq!(leaf_status, StatusCode::OK);
    assert_eq!(leaf_result["deleted_post_count"], 1);
    assert_eq!(deleted_state, 2);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn logged_in_user_favorites_visible_root_once_and_refreshes_count() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let favorite_count_before: Option<i32> =
        sqlx::query_scalar("SELECT favorite_count FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("user fixture should be readable");
    let (app, cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 5,
        },
    );

    let (created_status, created) = favorite_post(app.clone(), Some(&cookie), 106).await;
    let (duplicate_status, duplicate) = favorite_post(app, Some(&cookie), 106).await;
    let relation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM favorite WHERE user_id = 3 AND post_id = 106")
            .fetch_one(&pool)
            .await
            .expect("favorite relation should be readable");
    let updated_count: Option<i32> =
        sqlx::query_scalar("SELECT favorite_count FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("user statistic should be readable");

    sqlx::query("DELETE FROM favorite WHERE user_id = 3 AND post_id = 106")
        .execute(&pool)
        .await
        .expect("inserted favorite should be removed");
    sqlx::query("UPDATE user_info SET favorite_count = $1 WHERE id = 3")
        .bind(favorite_count_before)
        .execute(&pool)
        .await
        .expect("favorite count fixture should be restored");

    assert_eq!(created_status, StatusCode::CREATED);
    assert_eq!(created["favorited"], true);
    assert_eq!(created["favorite_count"], 1);
    assert_eq!(duplicate_status, StatusCode::CONFLICT);
    assert_eq!(duplicate["error"]["code"], "already_favorited");
    assert_eq!(relation_count, 1);
    assert_eq!(updated_count, Some(1));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn favorite_requires_login_and_rejects_non_root_posts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (member_app, cookie) = common::authenticated_test_app(pool);

    let (anonymous_status, anonymous) = favorite_post(public_app, None, 106).await;
    let (reply_status, reply) = favorite_post(member_app, Some(&cookie), 102).await;

    assert_eq!(anonymous_status, StatusCode::UNAUTHORIZED);
    assert_eq!(anonymous["error"]["code"], "authentication_required");
    assert_eq!(reply_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply["error"]["code"], "invalid_favorite_target");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_master_soft_deletes_an_entire_root_tree_and_refreshes_statistics() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let states_before: Vec<(i32, i32)> =
        sqlx::query_as("SELECT id, state FROM post WHERE id IN (101, 102, 105) ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("post fixtures should be readable");
    let board_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("board fixture should be readable");
    let users_before: Vec<(i32, i32, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT id, post_count, doc_count, favorite_count FROM user_info ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("user fixtures should be readable");
    let public_app = common::test_app(pool.clone());
    let (master_app, master_cookie) = common::authenticated_test_app(pool.clone());

    let (anonymous_status, _) = delete_post(public_app, None, 101).await;
    let (master_status, master_result) = delete_post(master_app, Some(&master_cookie), 101).await;
    let deleted_states: Vec<i32> =
        sqlx::query_scalar("SELECT state FROM post WHERE id IN (101, 102, 105) ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("tree states should be readable");
    let board_after: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("updated board statistic should be readable");
    let post_101_status = get_with_cookie(common::test_app(pool.clone()), "/api/posts/101", None)
        .await
        .0;
    let post_102_status = get_with_cookie(common::test_app(pool.clone()), "/api/posts/102", None)
        .await
        .0;

    for (post_id, state) in states_before {
        sqlx::query("UPDATE post SET state = $1 WHERE id = $2")
            .bind(state)
            .bind(post_id)
            .execute(&pool)
            .await
            .expect("post fixture should be restored");
    }
    sqlx::query("UPDATE board SET post_count = $1, root_count = $2 WHERE id = 11")
        .bind(board_before.0)
        .bind(board_before.1)
        .execute(&pool)
        .await
        .expect("board fixture should be restored");
    for (id, post_count, doc_count, favorite_count) in users_before {
        sqlx::query(
            "UPDATE user_info SET post_count = $1, doc_count = $2, favorite_count = $3 WHERE id = $4",
        )
        .bind(post_count)
        .bind(doc_count)
        .bind(favorite_count)
        .bind(id)
        .execute(&pool)
        .await
        .expect("user fixture should be restored");
    }

    assert_eq!(anonymous_status, StatusCode::UNAUTHORIZED);
    assert_eq!(master_status, StatusCode::OK);
    assert_eq!(master_result["deleted_post_count"], 3);
    assert_eq!(deleted_states, vec![2, 2, 2]);
    assert_eq!(board_after, (1, Some(1)));
    assert_eq!(post_101_status, StatusCode::NOT_FOUND);
    assert_eq!(post_102_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn logged_in_user_replies_immediately_after_parent_and_updates_tree_statistics() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool.clone());
    let root_before: (Option<i32>, Option<String>) = sqlx::query_as(
        "SELECT reply_count, to_char(reply_time, 'YYYY-MM-DD HH24:MI:SS.US') FROM post WHERE id = 101",
    )
            .fetch_one(&pool)
            .await
            .expect("root fixture should be readable");
    let shifted_before: i32 = sqlx::query_scalar("SELECT order_num FROM post WHERE id = 105")
        .fetch_one(&pool)
        .await
        .expect("nested reply fixture should be readable");
    let board_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("board fixture should be readable");
    let user_before: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count FROM user_info WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("user fixture should be readable");
    let tree_root_time_before: Option<String> = sqlx::query_scalar(
        "SELECT to_char(post_time, 'YYYY-MM-DD HH24:MI:SS.US') FROM post WHERE id = 101",
    )
    .fetch_one(&pool)
    .await
    .expect("tree root fixture should be readable");
    sqlx::query("UPDATE post SET post_time = CURRENT_TIMESTAMP WHERE id = 101")
        .execute(&pool)
        .await
        .expect("tree root should be within reply window");

    let (editor_status, editor) =
        get_with_cookie(app.clone(), "/api/post_upd?reply_to=102", Some(&cookie)).await;
    let (invalid_status, invalid) = save_post(
        app.clone(),
        Some(&cookie),
        r#"{"parent_id":102,"subject":"Invalid reply","content":"","post_type":2,"state":0}"#,
    )
    .await;
    let (save_status, saved) = save_post(
        app,
        Some(&cookie),
        r#"{"parent_id":102,"subject":"Reply child","content":"Encrypted reply body","state":1}"#,
    )
    .await;
    let post_id = saved["post_id"].as_i64().expect("reply post id") as i32;
    let reply: (i32, i32, i32, i32, Option<i32>, i32) = sqlx::query_as(
        "SELECT parent_id, root_id, level, order_num, type, state FROM post WHERE id = $1",
    )
    .bind(post_id)
    .fetch_one(&pool)
    .await
    .expect("reply should be readable");
    let shifted_after: i32 = sqlx::query_scalar("SELECT order_num FROM post WHERE id = 105")
        .fetch_one(&pool)
        .await
        .expect("nested reply should be readable");
    let root_after: (Option<i32>, Option<String>) = sqlx::query_as(
        "SELECT reply_count, to_char(reply_time, 'YYYY-MM-DD HH24:MI:SS.US') FROM post WHERE id = 101",
    )
            .fetch_one(&pool)
            .await
            .expect("root should be readable");
    let board_after: (i32, Option<i32>) =
        sqlx::query_as("SELECT post_count, root_count FROM board WHERE id = 11")
            .fetch_one(&pool)
            .await
            .expect("board should be readable");

    sqlx::query("DELETE FROM post WHERE id = $1")
        .bind(post_id)
        .execute(&pool)
        .await
        .expect("temporary reply should be removed");
    sqlx::query("UPDATE post SET order_num = $1 WHERE id = 105")
        .bind(shifted_before)
        .execute(&pool)
        .await
        .expect("tree ordering fixture should be restored");
    sqlx::query("UPDATE post SET reply_count = $1, reply_time = $2::timestamp WHERE id = 101")
        .bind(root_before.0)
        .bind(root_before.1.clone())
        .execute(&pool)
        .await
        .expect("root fixture should be restored");
    sqlx::query("UPDATE board SET post_count = $1, root_count = $2 WHERE id = 11")
        .bind(board_before.0)
        .bind(board_before.1)
        .execute(&pool)
        .await
        .expect("board fixture should be restored");
    sqlx::query("UPDATE user_info SET post_count = $1, doc_count = $2 WHERE id = 2")
        .bind(user_before.0)
        .bind(user_before.1)
        .execute(&pool)
        .await
        .expect("user fixture should be restored");
    sqlx::query("UPDATE post SET post_time = $1::timestamp WHERE id = 101")
        .bind(tree_root_time_before)
        .execute(&pool)
        .await
        .expect("tree root time fixture should be restored");

    assert_eq!(editor_status, StatusCode::OK);
    assert_eq!(editor["mode"], "reply");
    assert_eq!(editor["parent"]["subject"], "Original reply");
    assert_eq!(invalid_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid["error"]["code"], "invalid_post_option");
    assert_eq!(save_status, StatusCode::CREATED);
    assert_eq!(reply, (102, 101, 2, 2, Some(0), 1));
    assert_eq!(shifted_after, 3);
    assert_eq!(root_after.0, Some(4));
    assert!(root_after.1 > root_before.1);
    assert_eq!(board_after, (5, Some(2)));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn reply_editor_and_submission_reject_posts_outside_reply_window() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);

    let (editor_status, editor) =
        get_with_cookie(app.clone(), "/api/post_upd?reply_to=103", Some(&cookie)).await;
    let (save_status, save) = save_post(
        app,
        Some(&cookie),
        r#"{"parent_id":103,"subject":"Too late","content":"","state":0}"#,
    )
    .await;

    assert_eq!(editor_status, StatusCode::CONFLICT);
    assert_eq!(editor["error"]["code"], "reply_closed");
    assert_eq!(save_status, StatusCode::CONFLICT);
    assert_eq!(save["error"]["code"], "reply_closed");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn existing_image_attachment_cannot_be_replaced() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory =
        std::env::temp_dir().join(format!("dogn3-upload-test-{}-{unique}", std::process::id()));
    let state = AppState::new(
        pool.clone(),
        None,
        "Test Forum".to_string(),
        50,
        10,
        50,
        131_072,
        image_directory.clone(),
        32,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    );
    let token = state.sessions.create(AuthenticatedUser {
        id: 3,
        name: "Carol".to_string(),
        level: 5,
    });
    let cookie = format!("dogn_session={token}");
    let app = build_router(state);
    let image_bytes: &'static [u8] = b"\x89PNG\r\n\x1a\nuploaded-image";

    let (upload_status, body) =
        upload_image(app, &cookie, 103, "image/png", image_bytes.to_vec()).await;

    assert_eq!(upload_status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "image_update_not_allowed");
    assert!(!image_directory.exists());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn oversized_image_upload_is_stored_as_compressed_jpeg_below_threshold() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory = std::env::temp_dir().join(format!(
        "dogn3-compressed-upload-test-{}-{unique}",
        std::process::id()
    ));
    let original_image_url: Option<String> =
        sqlx::query_scalar("SELECT image_url FROM post WHERE id = 101")
            .fetch_one(&pool)
            .await
            .expect("post fixture should be readable");
    sqlx::query("UPDATE post SET image_url = NULL WHERE id = 101")
        .execute(&pool)
        .await
        .expect("post should accept its first managed attachment");
    let state = AppState::new(
        pool.clone(),
        None,
        "Test Forum".to_string(),
        50,
        10,
        50,
        131_072,
        image_directory.clone(),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    );
    let token = state.sessions.create(AuthenticatedUser {
        id: 2,
        name: "Bob".to_string(),
        level: 1,
    });
    let cookie = format!("dogn_session={token}");
    let app = build_router(state);
    let image = RgbImage::from_fn(700, 700, |x, y| {
        let seed = x
            .wrapping_mul(1_664_525)
            .wrapping_add(y.wrapping_mul(1_013_904_223));
        Rgb([
            seed as u8,
            (seed >> 8) as u8,
            (seed.rotate_left(11) >> 16) as u8,
        ])
    });
    let mut source = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut source), ImageFormat::Png)
        .expect("PNG fixture should encode");
    assert!(source.len() > 500 * 1024);
    assert!(source.len() < 2_097_152);

    let (status, response) = upload_image(app, &cookie, 101, "image/png", source).await;
    let stored_path = response["image_url"]
        .as_str()
        .expect("stored image path should be returned");
    let stored_body =
        fs::read(image_directory.join(stored_path)).expect("stored image should be readable");

    sqlx::query("UPDATE post SET image_url = $1 WHERE id = 101")
        .bind(original_image_url)
        .execute(&pool)
        .await
        .expect("post image fixture should be restored");
    fs::remove_dir_all(image_directory).expect("uploaded image fixture should be removed");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored_path, "uploads/post-101.jpg");
    assert_eq!(response["compressed"], true);
    assert!(
        response["stored_bytes"]
            .as_u64()
            .expect("stored byte count")
            < (500 * 1024) as u64
    );
    assert!(stored_body.len() < 500 * 1024);
    assert!(stored_body.starts_with(&[0xff, 0xd8, 0xff]));
}
