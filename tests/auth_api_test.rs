mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dogn3::{
    auth::{hash_migrated_input, legacy_password_input},
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
    let hash = hash_migrated_input(&legacy_password_input("frozen-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1', level = 0 WHERE id = 3",
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("frozen credential fixture should update");
    let app = common::test_app(pool);

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
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = response_json(response).await;
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
        PathBuf::from("images"),
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
