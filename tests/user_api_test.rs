mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::auth::{AuthenticatedUser, MODERN_PASSWORD_SCHEME, verify_modern_password};
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

async fn post_json_with_cookie(app: axum::Router, uri: &str, cookie: &str) -> (StatusCode, Value) {
    post_json_body_with_cookie(app, uri, cookie, serde_json::json!({})).await
}

async fn post_json_body_with_cookie(
    app: axum::Router,
    uri: &str,
    cookie: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::COOKIE, cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_list_endpoint_requires_administrator() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let public_app = common::test_app(pool.clone());
    let (member_app, member_cookie) = common::authenticated_test_app(pool);

    let (public_status, public_body) = get_json(public_app, "/api/users").await;
    let (member_status, member_body) =
        get_json_with_cookie(member_app, "/api/users", Some(&member_cookie)).await;

    assert_eq!(public_status, StatusCode::UNAUTHORIZED);
    assert_eq!(public_body["error"]["code"], "authentication_required");
    assert_eq!(member_status, StatusCode::FORBIDDEN);
    assert_eq!(member_body["error"]["code"], "not_authorized");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn downgraded_administrator_loses_directory_profile_and_statistics_privileges() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );
    sqlx::query("UPDATE user_info SET level = 1 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("administrator fixture should be downgraded");

    let (list_status, list_body) =
        get_json_with_cookie(app.clone(), "/api/users", Some(&admin_cookie)).await;
    let (profile_status, profile_body) =
        get_json_with_cookie(app.clone(), "/api/users/2", Some(&admin_cookie)).await;
    let (statistics_status, statistics_body) =
        post_json_with_cookie(app, "/api/users/2/statistics/recalculate", &admin_cookie).await;

    sqlx::query("UPDATE user_info SET level = 10 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("administrator fixture should be restored");
    assert_eq!(list_status, StatusCode::FORBIDDEN);
    assert_eq!(list_body["error"]["code"], "not_authorized");
    assert_eq!(profile_status, StatusCode::OK);
    assert_eq!(profile_body["can_update"], false);
    assert!(profile_body.get("private_details").is_none());
    assert_eq!(statistics_status, StatusCode::FORBIDDEN);
    assert_eq!(statistics_body["error"]["code"], "not_authorized");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_can_search_sort_and_page_user_list() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let (status, page) = get_json_with_cookie(
        app.clone(),
        "/api/users?order=id_asc&page_size=2&page=1",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["order"], "id_asc");
    assert_eq!(page["pager"]["total_users"], 3);
    assert_eq!(page["pager"]["page_size"], 2);
    assert_eq!(page["pager"]["total_pages"], 2);
    assert_eq!(page["users"][0]["id"], 1);
    assert_eq!(page["users"][1]["id"], 2);
    assert_eq!(page["users"][0]["email"], "alice@example.test");

    let (status, search) = get_json_with_cookie(
        app.clone(),
        "/api/users?query=BOB%40EXAMPLE.TEST&order=id_desc",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(search["query"], "BOB@EXAMPLE.TEST");
    assert_eq!(search["pager"]["total_users"], 1);
    assert_eq!(search["users"][0]["id"], 2);
    assert_eq!(search["users"][0]["name"], "Bob");

    let (status, role) =
        get_json_with_cookie(app, "/api/users?role=10&order=id_desc", Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(role["role"], 10);
    assert_eq!(role["pager"]["total_users"], 1);
    assert_eq!(role["users"][0]["id"], 1);
    assert_eq!(role["users"][0]["level"], 10);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_can_create_a_user_with_a_modern_password() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app_as(
        pool.clone(),
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let (status, body) = post_json_body_with_cookie(
        app,
        "/api/users",
        &cookie,
        serde_json::json!({
            "name": "New member",
            "email": "new@example.test",
            "level": 1,
            "password": "Forum123!",
            "confirm_password": "Forum123!"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["created"], true);
    let user_id = body["user_id"].as_i64().expect("created user id") as i32;
    let stored: (String, Option<String>, i32, Option<String>) = sqlx::query_as(
        "SELECT password, password_scheme, level, BTRIM(email) FROM user_info WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("created user should exist");
    assert_eq!(stored.1.as_deref(), Some(MODERN_PASSWORD_SCHEME));
    assert!(verify_modern_password("Forum123!", &stored.0));
    assert_eq!(stored.2, 1);
    assert_eq!(stored.3.as_deref(), Some("new@example.test"));

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("created user should be removed after assertion");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn create_user_requires_administrator_and_rejects_duplicate_names() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (member_app, member_cookie) = common::authenticated_test_app(pool.clone());
    let (admin_app, admin_cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );
    let body = serde_json::json!({
        "name": "Alice",
        "email": "",
        "level": 1,
        "password": "Forum123!",
        "confirm_password": "Forum123!"
    });

    let (member_status, member_body) =
        post_json_body_with_cookie(member_app, "/api/users", &member_cookie, body.clone()).await;
    let (duplicate_status, duplicate_body) =
        post_json_body_with_cookie(admin_app, "/api/users", &admin_cookie, body).await;

    assert_eq!(member_status, StatusCode::FORBIDDEN);
    assert_eq!(member_body["error"]["code"], "not_authorized");
    assert_eq!(duplicate_status, StatusCode::CONFLICT);
    assert_eq!(duplicate_body["error"]["code"], "duplicate_user_name");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn recalculate_statistics_updates_readable_counts_for_owner() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    sqlx::query(
        "UPDATE user_info SET post_count = 999, doc_count = 999, favorite_count = 999 WHERE id = 2",
    )
    .execute(&pool)
    .await
    .expect("fixture should become stale");
    let (app, cookie) = common::authenticated_test_app(pool.clone());

    let (status, body) =
        post_json_with_cookie(app, "/api/users/2/statistics/recalculate", &cookie).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user_id"], 2);
    assert_eq!(body["post_count"], 1);
    assert_eq!(body["doc_count"], 1);
    assert_eq!(body["favorite_count"], 2);

    let stored: (i32, Option<i32>, Option<i32>) =
        sqlx::query_as("SELECT post_count, doc_count, favorite_count FROM user_info WHERE id = 2")
            .fetch_one(&pool)
            .await
            .expect("statistics should be readable");
    assert_eq!(stored, (1, Some(1), Some(2)));

    sqlx::query(
        "UPDATE user_info SET post_count = 6, doc_count = 3, favorite_count = 2 WHERE id = 2",
    )
    .execute(&pool)
    .await
    .expect("fixture should be restored");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn recalculate_statistics_rejects_other_members() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 3,
            name: "Carol".to_string(),
            level: 1,
        },
    );

    let (status, body) =
        post_json_with_cookie(app, "/api/users/2/statistics/recalculate", &cookie).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "not_authorized");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_can_recalculate_another_users_statistics() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let (status, body) =
        post_json_with_cookie(app, "/api/users/2/statistics/recalculate", &cookie).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user_id"], 2);
    assert_eq!(body["post_count"], 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn recalculation_requires_mutation_request_header() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/2/statistics/recalculate")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&body).expect("response should be json");
    assert_eq!(body["error"]["code"], "csrf_check_failed");
}
