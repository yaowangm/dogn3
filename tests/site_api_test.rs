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

async fn post_json(
    app: axum::Router,
    uri: &str,
    cookie: &str,
    body: &'static str,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::COOKIE, cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
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
    let board: (String, Option<String>, i32, i32, Option<String>, Option<String>, Option<String>, Option<String>, i32, Option<i32>) =
        sqlx::query_as(
            "SELECT name, comment, category_id, order_id, master_name, master_name_2, master_name_3, master_name_4, post_count, root_count FROM board WHERE id = 20",
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
        r#"{"name":"Rust Lang","comment":"Modern Rust","category_id":1,"order_id":3,"master_names":["Alice","Bob","",""]}"#,
    )
    .await;
    sqlx::query("UPDATE board SET post_count = 999, root_count = 999 WHERE id = 20")
        .execute(&pool)
        .await
        .expect("board statistics should become stale");
    let (statistics_status, statistics) = post_json(
        app.clone(),
        "/api/site_manager/boards/20/statistics/recalculate",
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
        "UPDATE board SET name = $1, comment = $2, category_id = $3, order_id = $4, master_name = $5, master_name_2 = $6, master_name_3 = $7, master_name_4 = $8, post_count = $9, root_count = $10 WHERE id = 20",
    )
    .bind(board.0)
    .bind(board.1)
    .bind(board.2)
    .bind(board.3)
    .bind(board.4)
    .bind(board.5)
    .bind(board.6)
    .bind(board.7)
    .bind(board.8)
    .bind(board.9)
    .execute(&pool)
    .await
    .expect("board fixture should be restored");

    assert_eq!(category_status, StatusCode::OK);
    assert_eq!(board_status, StatusCode::OK);
    assert_eq!(statistics_status, StatusCode::OK);
    assert_eq!(statistics["post_count"], 1);
    assert_eq!(statistics["root_count"], 1);
    assert_eq!(updated["categories"][1]["name"], "Engineering");
    let updated_board = updated["boards"]
        .as_array()
        .expect("boards")
        .iter()
        .find(|board| board["id"] == 20)
        .expect("updated board");
    assert_eq!(updated_board["name"], "Rust Lang");
    assert_eq!(updated_board["category_id"], 1);
}
