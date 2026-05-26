mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::{
    auth::{AuthenticatedUser, MODERN_PASSWORD_SCHEME, hash_migrated_input, legacy_password_input},
    build_router,
    state::{AppState, AuthRuntimeConfig},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{path::PathBuf, time::Duration};
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_creates_session_and_logout_clears_it() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let hash = hash_migrated_input(&legacy_password_input("test-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1', state = 1 WHERE id = 2",
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("credential fixture should update");
    let app = common::test_app(pool);

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"Bob","password":"test-password"}"#))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .expect("cookie should be utf-8")
        .to_string();
    assert!(cookie.contains("dogn_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("Secure"));
    let login_body = response_json(login).await;
    assert_eq!(login_body["authenticated"], true);
    assert_eq!(login_body["user"]["name"], "Bob");

    let session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let session_body = response_json(session).await;
    assert_eq!(session_body["authenticated"], true);
    assert_eq!(session_body["user"]["id"], 2);

    let logout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, &cookie)
                .body(Body::from("{}"))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(logout.status(), StatusCode::OK);
    assert!(
        logout.headers()[header::SET_COOKIE]
            .to_str()
            .expect("cookie should be utf-8")
            .contains("Max-Age=0")
    );

    let after_logout = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(response_json(after_logout).await["authenticated"], false);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_returns_generic_failure_for_invalid_unmigrated_or_frozen_credentials() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original: (String, Option<String>, i32) =
        sqlx::query_as("SELECT password, password_scheme, level FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("original credential fixture should be readable");
    let hash = hash_migrated_input(&legacy_password_input("frozen-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1', level = 0 WHERE id = 3",
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("frozen credential fixture should update");
    let app = common::test_app(pool.clone());

    let mut responses = Vec::new();
    for body in [
        r#"{"name":"Alice","password":"wrong"}"#,
        r#"{"name":"Nobody","password":"wrong"}"#,
        r#"{"name":"Carol","password":"frozen-password"}"#,
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .expect("valid request"),
            )
            .await
            .expect("route should respond");
        let status = response.status();
        responses.push((status, response_json(response).await));
    }

    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = $2, level = $3 WHERE id = 3",
    )
    .bind(original.0)
    .bind(original.1)
    .bind(original.2)
    .execute(&pool)
    .await
    .expect("frozen credential fixture should be restored");

    for (status, response) in responses {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(response["error"]["code"], "invalid_credentials");
        assert_eq!(
            response["error"]["message"],
            "Invalid user name or password."
        );
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_rejects_work_when_password_hash_capacity_is_exhausted() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let state = AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        10,
        50,
        131_072,
        PathBuf::from("images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 1,
        },
    );
    let _permit = state
        .login_hash_permits
        .clone()
        .acquire_owned()
        .await
        .expect("permit should be available");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"Nobody","password":"wrong"}"#))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    assert_eq!(response_json(response).await["error"]["code"], "login_busy");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn owner_can_change_password_and_is_logged_out() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let current_hash =
        hash_migrated_input(&legacy_password_input("current-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1' WHERE id = 2",
    )
    .bind(current_hash)
    .execute(&pool)
    .await
    .expect("credential fixture should update");
    let (app, cookie) = common::authenticated_test_app(pool.clone());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/2/password")
                .header(header::COOKIE, &cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"current-password","new_password":"NewForum2!","confirm_password":"NewForum2!"}"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["changed"], true);
    assert_eq!(body["session_invalidated"], true);
    let scheme: String = sqlx::query_scalar("SELECT password_scheme FROM user_info WHERE id = 2")
        .fetch_one(&pool)
        .await
        .expect("updated scheme should be readable");
    assert_eq!(scheme, MODERN_PASSWORD_SCHEME);

    let session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(response_json(session).await["authenticated"], false);

    let login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"Bob","password":"NewForum2!"}"#))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(login.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_can_reset_another_password_without_current_password() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, admin_cookie) = common::authenticated_test_app_as(
        pool,
        AuthenticatedUser {
            id: 1,
            name: "Alice".to_string(),
            level: 10,
        },
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/3/password")
                .header(header::COOKIE, &admin_cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"new_password":"ResetCarol3!","confirm_password":"ResetCarol3!"}"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["session_invalidated"], false);
    let admin_session = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &admin_cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(response_json(admin_session).await["authenticated"], true);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn downgraded_administrator_cannot_reset_another_password() {
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

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/3/password")
                .header(header::COOKIE, &admin_cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"new_password":"ShouldNotChange3!","confirm_password":"ShouldNotChange3!"}"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    sqlx::query("UPDATE user_info SET level = 10 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("administrator fixture should be restored");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "not_authorized"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn session_uses_current_account_level_and_rejects_frozen_accounts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool.clone());
    sqlx::query("UPDATE user_info SET level = 5 WHERE id = 2")
        .execute(&pool)
        .await
        .expect("member fixture should be promoted");

    let promoted = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    sqlx::query("UPDATE user_info SET level = 0 WHERE id = 2")
        .execute(&pool)
        .await
        .expect("member fixture should be frozen");
    let frozen = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    sqlx::query("UPDATE user_info SET level = 5 WHERE id = 2")
        .execute(&pool)
        .await
        .expect("board-master fixture should be restored");
    let promoted = response_json(promoted).await;
    let frozen = response_json(frozen).await;
    assert_eq!(promoted["authenticated"], true);
    assert_eq!(promoted["user"]["level"], 5);
    assert_eq!(frozen["authenticated"], false);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_change_rejects_unauthorized_or_unverified_requests() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);

    let cross_account = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/1/password")
                .header(header::COOKIE, &cookie)
                .header("x-dogn-request", "fetch")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"anything","new_password":"Blocked1!","confirm_password":"Blocked1!"}"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(cross_account.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(cross_account).await["error"]["code"],
        "not_authorized"
    );

    let no_csrf_header = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/2/password")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"anything","new_password":"Blocked1!","confirm_password":"Blocked1!"}"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(no_csrf_header.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(no_csrf_header).await["error"]["code"],
        "csrf_check_failed"
    );
}
