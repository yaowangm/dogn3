mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::auth::AuthenticatedUser;
use http_body_util::BodyExt;
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_editor_requires_login_for_create_and_update() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (create_get, create_body) =
        get_with_cookie(app.clone(), "/api/post_upd?board_id=11", None).await;
    let (update_get, _) = get_with_cookie(app.clone(), "/api/post_upd?post_id=101", None).await;
    let (save_status, _) = save_post(
        app,
        None,
        r#"{"board_id":11,"subject":"New","content":"Text","post_type":0,"state":0}"#,
    )
    .await;

    assert_eq!(create_get, StatusCode::UNAUTHORIZED);
    assert_eq!(create_body["error"]["code"], "authentication_required");
    assert_eq!(update_get, StatusCode::UNAUTHORIZED);
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
        r#"{"board_id":11,"subject":"Created root","content":"Created body","post_type":1,"state":0,"link_name":"Docs","link_url":"https://example.test/docs","image_url":""}"#,
    )
    .await;
    let post_id = saved["post_id"].as_i64().expect("created post id") as i32;
    let post: (i32, i32, i32, i32, i32, i32) = sqlx::query_as(
        "SELECT user_id, parent_id, root_id, level, order_num, reply_count FROM post WHERE id = $1",
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
    assert_eq!(save_status, StatusCode::CREATED);
    assert_eq!(post, (2, 0, post_id, 0, 0, 1));
    assert_eq!(board_after, (5, Some(3)));
    assert_eq!(user_after, (2, Some(2)));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn only_post_owner_or_administrator_can_update_post() {
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
    let updated_subject: Option<String> =
        sqlx::query_scalar("SELECT subject FROM post WHERE id = 101")
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
    assert_eq!(updated_subject.as_deref(), Some("Admin update"));
}
