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

async fn post_json(app: axum::Router, uri: &str, cookie: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::COOKIE, cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

fn admin_app(pool: sqlx::PgPool) -> (axum::Router, String) {
    common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    )
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn site_manager_requires_administrator() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (member_app, member_cookie) = common::authenticated_test_app(pool);

    let (public_status, public_body) = get_with_cookie(public_app, "/api/site_manager", None).await;
    let (member_status, member_body) =
        get_with_cookie(member_app, "/api/site_manager", Some(&member_cookie)).await;

    assert_eq!(public_status, StatusCode::UNAUTHORIZED);
    assert_eq!(public_body["error"]["code"], "authentication_required");
    assert_eq!(member_status, StatusCode::FORBIDDEN);
    assert_eq!(member_body["error"]["code"], "not_authorized");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn site_manager_returns_categories_and_boards_for_administrator() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = admin_app(pool);

    let (status, body) = get_with_cookie(app, "/api/site_manager", Some(&cookie)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["site_name"], "Test Forum");
    assert_eq!(body["categories"][0]["name"], "General");
    assert_eq!(body["categories"][0]["board_count"], 2);
    assert_eq!(body["boards"].as_array().expect("boards").len(), 3);
    assert!(body.get("master_users").is_none());
    assert_eq!(
        body["boards"][1]["masters"],
        serde_json::json!([{"id": 2, "name": "Bob"}, {"id": 3, "name": "Carol"}])
    );
    assert_eq!(
        body["navigation_boards"]
            .as_array()
            .expect("navigation")
            .len(),
        3
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_updates_site_metadata_and_recalculates_board_statistics() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let category: (String, Option<String>, i32) =
        sqlx::query_as("SELECT name, comment, order_id FROM category WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("category fixture should be readable");
    let board: (String, Option<String>, i32, i32, i32, Option<i32>) =
        sqlx::query_as(
            "SELECT name, comment, category_id, order_id, post_count, root_count FROM board WHERE id = 20",
        )
        .fetch_one(&pool)
        .await
        .expect("board fixture should be readable");
    let (app, cookie) = admin_app(pool.clone());

    let (category_status, _) = post_json(
        app.clone(),
        "/api/site_manager/categories/2",
        &cookie,
        r#"{"name":"Engineering","comment":"Engineering boards","order_id":4}"#,
    )
    .await;
    let (board_status, _) = post_json(
        app.clone(),
        "/api/site_manager/boards/20",
        &cookie,
        r#"{"name":"Rust Lang","comment":"Modern Rust","category_id":1,"order_id":3}"#,
    )
    .await;
    sqlx::query("UPDATE board SET post_count = 999, root_count = 999 WHERE id = 20")
        .execute(&pool)
        .await
        .expect("board statistics should become stale");
    let (statistics_status, statistics) = post_json(
        app.clone(),
        "/api/site_manager/boards/statistics/recalculate",
        &cookie,
        "{}",
    )
    .await;
    let (_, updated) = get_with_cookie(app, "/api/site_manager", Some(&cookie)).await;

    sqlx::query("UPDATE category SET name = $1, comment = $2, order_id = $3 WHERE id = 2")
        .bind(category.0)
        .bind(category.1)
        .bind(category.2)
        .execute(&pool)
        .await
        .expect("category fixture should be restored");
    sqlx::query(
        "UPDATE board SET name = $1, comment = $2, category_id = $3, order_id = $4, post_count = $5, root_count = $6 WHERE id = 20",
    )
    .bind(board.0)
    .bind(board.1)
    .bind(board.2)
    .bind(board.3)
    .bind(board.4)
    .bind(board.5)
    .execute(&pool)
    .await
    .expect("board fixture should be restored");
    sqlx::query("DELETE FROM board_master WHERE board_id = 20")
        .execute(&pool)
        .await
        .expect("board master fixture should be restored");

    assert_eq!(category_status, StatusCode::OK);
    assert_eq!(board_status, StatusCode::OK);
    assert_eq!(statistics_status, StatusCode::OK);
    assert_eq!(statistics["updated_boards"], 3);
    assert_eq!(updated["categories"][1]["name"], "Engineering");
    let updated_board = updated["boards"]
        .as_array()
        .expect("boards")
        .iter()
        .find(|board| board["id"] == 20)
        .expect("updated board");
    assert_eq!(updated_board["name"], "Rust Lang");
    assert_eq!(updated_board["category_id"], 1);
    assert_eq!(updated_board["post_count"], 1);
    assert_eq!(updated_board["root_count"], 1);
    assert_eq!(updated_board["masters"], serde_json::json!([]));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_adds_and_removes_board_masters_immediately() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = admin_app(pool.clone());

    let (add_status, added) = post_json(
        app.clone(),
        "/api/site_manager/boards/20/masters",
        &cookie,
        r#"{"user_id":1}"#,
    )
    .await;
    let (duplicate_status, duplicate) = post_json(
        app.clone(),
        "/api/site_manager/boards/20/masters",
        &cookie,
        r#"{"user_id":1}"#,
    )
    .await;
    let (remove_status, removed) = post_json(
        app,
        "/api/site_manager/boards/20/masters/1/remove",
        &cookie,
        "{}",
    )
    .await;
    let assignments = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM board_master WHERE board_id = 20 AND user_id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("board master relationship should be readable");

    assert_eq!(add_status, StatusCode::OK);
    assert_eq!(added["master"]["id"], 1);
    assert_eq!(duplicate_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(duplicate["error"]["code"], "duplicate_master");
    assert_eq!(remove_status, StatusCode::OK);
    assert_eq!(removed["master"]["id"], 1);
    assert_eq!(assignments, 0);
    let admin_level: i32 = sqlx::query_scalar("SELECT level FROM user_info WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("administrator level should be readable");
    assert_eq!(admin_level, 10);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_master_changes_automatically_manage_advanced_member_level() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = admin_app(pool.clone());
    let user_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, level) VALUES ('Master transition', 'fixture', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("temporary member should be created");

    let (add_status, _) = post_json(
        app.clone(),
        "/api/site_manager/boards/20/masters",
        &cookie,
        &format!(r#"{{"user_id":{user_id}}}"#),
    )
    .await;
    let promoted: i32 = sqlx::query_scalar("SELECT level FROM user_info WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("promoted level should be readable");
    let (second_add_status, _) = post_json(
        app.clone(),
        "/api/site_manager/boards/10/masters",
        &cookie,
        &format!(r#"{{"user_id":{user_id}}}"#),
    )
    .await;
    let (first_remove_status, _) = post_json(
        app.clone(),
        &format!("/api/site_manager/boards/20/masters/{user_id}/remove"),
        &cookie,
        "{}",
    )
    .await;
    let retained: i32 = sqlx::query_scalar("SELECT level FROM user_info WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("retained level should be readable");
    let (last_remove_status, _) = post_json(
        app,
        &format!("/api/site_manager/boards/10/masters/{user_id}/remove"),
        &cookie,
        "{}",
    )
    .await;
    let demoted: i32 = sqlx::query_scalar("SELECT level FROM user_info WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("demoted level should be readable");

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("temporary member should be removed");

    assert_eq!(add_status, StatusCode::OK);
    assert_eq!(promoted, 5);
    assert_eq!(second_add_status, StatusCode::OK);
    assert_eq!(first_remove_status, StatusCode::OK);
    assert_eq!(retained, 5);
    assert_eq!(last_remove_status, StatusCode::OK);
    assert_eq!(demoted, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_creates_and_deletes_only_empty_categories_and_boards() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = admin_app(pool);

    let (create_category_status, created_category) = post_json(
        app.clone(),
        "/api/site_manager/categories",
        &cookie,
        r#"{"name":"Temporary","comment":"Temporary category","order_id":99}"#,
    )
    .await;
    let category_id = created_category["target_id"]
        .as_i64()
        .expect("created category id");
    let (create_board_status, created_board) = post_json(
        app.clone(),
        "/api/site_manager/boards",
        &cookie,
        &format!(
            r#"{{"name":"Temporary Board","comment":"","category_id":{category_id},"order_id":1}}"#
        ),
    )
    .await;
    let board_id = created_board["target_id"]
        .as_i64()
        .expect("created board id");
    let (non_empty_category_status, non_empty_category) = post_json(
        app.clone(),
        &format!("/api/site_manager/categories/{category_id}/delete"),
        &cookie,
        "{}",
    )
    .await;
    let (non_empty_board_status, non_empty_board) = post_json(
        app.clone(),
        "/api/site_manager/boards/20/delete",
        &cookie,
        "{}",
    )
    .await;
    let (delete_board_status, _) = post_json(
        app.clone(),
        &format!("/api/site_manager/boards/{board_id}/delete"),
        &cookie,
        "{}",
    )
    .await;
    let (delete_category_status, _) = post_json(
        app,
        &format!("/api/site_manager/categories/{category_id}/delete"),
        &cookie,
        "{}",
    )
    .await;

    assert_eq!(create_category_status, StatusCode::CREATED);
    assert_eq!(create_board_status, StatusCode::CREATED);
    assert_eq!(non_empty_category_status, StatusCode::CONFLICT);
    assert_eq!(
        non_empty_category["error"]["code"],
        serde_json::json!("category_not_empty")
    );
    assert_eq!(non_empty_board_status, StatusCode::CONFLICT);
    assert_eq!(
        non_empty_board["error"]["code"],
        serde_json::json!("board_not_empty")
    );
    assert_eq!(delete_board_status, StatusCode::OK);
    assert_eq!(delete_category_status, StatusCode::OK);
}
