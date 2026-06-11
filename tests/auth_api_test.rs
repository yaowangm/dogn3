mod common;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header},
};
use dogn3::{
    auth::{AuthenticatedUser, MODERN_PASSWORD_SCHEME, hash_migrated_input, legacy_password_input},
    build_router,
    rate_limit::{RateLimitBackend, RateLimitConfig},
    state::{AppState, AuthRuntimeConfig, MailDelivery, PasswordResetConfig},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    time::timeout,
};
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

async fn post_json(app: axum::Router, uri: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-dogn-request", "fetch")
                .body(Body::from(body.to_owned()))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let status = response.status();
    (status, response_json(response).await)
}

fn enabled_reset_config(sendmail_path: PathBuf) -> PasswordResetConfig {
    PasswordResetConfig {
        enabled: true,
        mail_delivery: MailDelivery::Sendmail,
        sendmail_path,
        smtp_host: "127.0.0.1".to_string(),
        smtp_port: 25,
        mail_from: Some("no-reply@example.test".to_string()),
        public_site_url: Some("https://forum.example.test".to_string()),
        ttl: Duration::from_secs(1800),
    }
}

fn enabled_smtp_reset_config(port: u16) -> PasswordResetConfig {
    PasswordResetConfig {
        enabled: true,
        mail_delivery: MailDelivery::Smtp,
        sendmail_path: PathBuf::from("/usr/sbin/sendmail"),
        smtp_host: "127.0.0.1".to_string(),
        smtp_port: port,
        mail_from: Some("no-reply@example.test".to_string()),
        public_site_url: Some("https://forum.example.test".to_string()),
        ttl: Duration::from_secs(1800),
    }
}

fn reset_test_app(pool: sqlx::PgPool, sendmail_path: PathBuf) -> axum::Router {
    reset_test_app_with_rate_limit(pool, sendmail_path, RateLimitConfig::disabled())
}

fn reset_test_app_with_rate_limit(
    pool: sqlx::PgPool,
    sendmail_path: PathBuf,
    rate_limit: RateLimitConfig,
) -> axum::Router {
    reset_test_app_with_config(pool, enabled_reset_config(sendmail_path), rate_limit)
}

fn reset_test_app_with_config(
    pool: sqlx::PgPool,
    password_reset: PasswordResetConfig,
    rate_limit: RateLimitConfig,
) -> axum::Router {
    build_router(AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        10,
        100,
        100,
        2,
        5,
        10,
        50,
        131_072,
        1_000,
        PathBuf::from("images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
        password_reset,
        rate_limit,
    ))
}

fn auth_test_app_with_rate_limit(pool: sqlx::PgPool, rate_limit: RateLimitConfig) -> axum::Router {
    build_router(AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        10,
        100,
        100,
        2,
        5,
        10,
        50,
        131_072,
        1_000,
        PathBuf::from("images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
        common::disabled_password_reset_config(),
        rate_limit,
    ))
}

fn memory_rate_limit_config() -> RateLimitConfig {
    RateLimitConfig {
        enabled: true,
        backend: RateLimitBackend::Memory,
        login_fail_window: Duration::from_secs(900),
        login_fail_max_per_user: 1,
        login_fail_max_per_ip: 100,
        login_fail_lock: Duration::from_secs(900),
        password_reset_window: Duration::from_secs(3600),
        password_reset_max_per_email: 1,
        password_reset_max_per_ip: 100,
        password_reset_confirm_window: Duration::from_secs(900),
        password_reset_confirm_max_per_ip: 1,
    }
}

fn sendmail_fixture() -> (PathBuf, PathBuf) {
    let unique = unique_suffix();
    let directory = std::env::temp_dir().join(format!(
        "dogn3-sendmail-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("create sendmail fixture directory");
    let capture_path = directory.join("message.txt");
    let script_path = directory.join("sendmail");
    fs::write(
        &script_path,
        format!("#!/usr/bin/env bash\ncat > '{}'\n", capture_path.display()),
    )
    .expect("write sendmail fixture");
    let mut permissions = fs::metadata(&script_path)
        .expect("sendmail fixture metadata")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&script_path, permissions).expect("make sendmail fixture executable");
    (script_path, capture_path)
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos()
}

fn reset_token_from_message(message: &str) -> String {
    let (_, token) = message
        .split_once("token=")
        .expect("reset message should contain token");
    token
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .collect()
}

async fn smtp_fixture() -> (u16, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("SMTP fixture should bind");
    let port = listener.local_addr().expect("SMTP fixture address").port();
    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("SMTP fixture should accept");
        let mut smtp = BufReader::new(stream);
        smtp.get_mut()
            .write_all(b"220 smtp fixture\r\n")
            .await
            .expect("SMTP greeting should write");
        let mut message = String::new();
        let mut in_data = false;

        loop {
            let mut line = String::new();
            let bytes = smtp
                .read_line(&mut line)
                .await
                .expect("SMTP command should read");
            if bytes == 0 {
                break;
            }

            if in_data {
                if line == ".\r\n" || line == ".\n" {
                    in_data = false;
                    smtp.get_mut()
                        .write_all(b"250 accepted\r\n")
                        .await
                        .expect("SMTP data response should write");
                } else {
                    message.push_str(&line);
                }
                continue;
            }

            let command = line.to_ascii_uppercase();
            if command.starts_with("EHLO") || command.starts_with("HELO") {
                smtp.get_mut()
                    .write_all(b"250-localhost\r\n250 OK\r\n")
                    .await
                    .expect("SMTP EHLO response should write");
            } else if command.starts_with("MAIL FROM:") || command.starts_with("RCPT TO:") {
                smtp.get_mut()
                    .write_all(b"250 OK\r\n")
                    .await
                    .expect("SMTP envelope response should write");
            } else if command.starts_with("DATA") {
                in_data = true;
                smtp.get_mut()
                    .write_all(b"354 End data\r\n")
                    .await
                    .expect("SMTP data prompt should write");
            } else if command.starts_with("QUIT") {
                smtp.get_mut()
                    .write_all(b"221 Bye\r\n")
                    .await
                    .expect("SMTP quit response should write");
                break;
            } else {
                smtp.get_mut()
                    .write_all(b"500 Unknown\r\n")
                    .await
                    .expect("SMTP error response should write");
            }
        }

        message
    });

    (port, handle)
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_creates_session_and_logout_clears_it() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let activity_before: (Option<String>, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT to_char(last_login, 'YYYY-MM-DD HH24:MI:SS.US'), last_login_ip, login_count FROM user_info WHERE id = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("login activity fixture should be readable");
    let hash = hash_migrated_input(&legacy_password_input("test-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1', state = 1 WHERE id = 2",
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("credential fixture should update");
    let app = common::test_app(pool.clone());

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .extension(ConnectInfo(SocketAddr::from(([203, 0, 113, 12], 45678))))
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
    assert!(login_body["expires_at_epoch_ms"].as_u64().is_some());
    let activity_after: (Option<String>, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT to_char(last_login, 'YYYY-MM-DD HH24:MI:SS.US'), last_login_ip, login_count FROM user_info WHERE id = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("updated login activity should be readable");
    assert!(activity_after.0 > activity_before.0);
    assert_eq!(activity_after.1.as_deref(), Some("203.0.113.12"));
    assert_eq!(activity_after.2, activity_before.2.map(|count| count + 1));

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
    assert_eq!(
        session_body["expires_at_epoch_ms"],
        login_body["expires_at_epoch_ms"]
    );

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
    sqlx::query(
        "UPDATE user_info SET last_login = $1::timestamp, last_login_ip = $2, login_count = $3 WHERE id = 2",
    )
    .bind(activity_before.0)
    .bind(activity_before.1)
    .bind(activity_before.2)
    .execute(&pool)
    .await
    .expect("login activity fixture should be restored");
    let after_logout_body = response_json(after_logout).await;
    assert_eq!(after_logout_body["authenticated"], false);
    assert!(after_logout_body["expires_at_epoch_ms"].is_null());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_returns_specific_failure_for_frozen_accounts_only() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let alice_errors_before: (Option<String>, i32) = sqlx::query_as(
        "SELECT to_char(log_error_time, 'YYYY-MM-DD HH24:MI:SS.US'), log_error_count FROM user_info WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("administrator failure fixture should be readable");
    let original: (String, Option<String>, i32, Option<String>, i32) =
        sqlx::query_as(
            "SELECT password, password_scheme, level, to_char(log_error_time, 'YYYY-MM-DD HH24:MI:SS.US'), log_error_count FROM user_info WHERE id = 3",
        )
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

    let alice_errors_after: (Option<String>, i32) = sqlx::query_as(
        "SELECT to_char(log_error_time, 'YYYY-MM-DD HH24:MI:SS.US'), log_error_count FROM user_info WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("administrator failure state should be readable");
    let carol_errors_after: (Option<String>, i32) = sqlx::query_as(
        "SELECT to_char(log_error_time, 'YYYY-MM-DD HH24:MI:SS.US'), log_error_count FROM user_info WHERE id = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("frozen failure state should be readable");
    sqlx::query(
        "UPDATE user_info SET log_error_time = $1::timestamp, log_error_count = $2 WHERE id = 1",
    )
    .bind(alice_errors_before.0.clone())
    .bind(alice_errors_before.1)
    .execute(&pool)
    .await
    .expect("administrator failure fixture should be restored");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = $2, level = $3, log_error_time = $4::timestamp, log_error_count = $5 WHERE id = 3",
    )
    .bind(original.0)
    .bind(original.1)
    .bind(original.2)
    .bind(original.3.clone())
    .bind(original.4)
    .execute(&pool)
    .await
    .expect("frozen credential fixture should be restored");

    for (status, response) in responses.iter().take(2) {
        assert_eq!(*status, StatusCode::UNAUTHORIZED);
        assert_eq!(response["error"]["code"], "invalid_credentials");
        assert_eq!(
            response["error"]["message"],
            "Invalid user name or password."
        );
    }
    assert_eq!(responses[2].0, StatusCode::UNAUTHORIZED);
    assert_eq!(responses[2].1["error"]["code"], "account_frozen");
    assert_eq!(
        responses[2].1["error"]["message"],
        "This account is frozen. Contact an administrator."
    );
    assert!(alice_errors_after.0 > alice_errors_before.0);
    assert_eq!(alice_errors_after.1, alice_errors_before.1 + 1);
    assert!(carol_errors_after.0 > original.3);
    assert_eq!(carol_errors_after.1, original.4 + 1);
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
        100,
        100,
        2,
        5,
        10,
        50,
        131_072,
        1_000,
        PathBuf::from("images"),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 1,
        },
        common::disabled_password_reset_config(),
        RateLimitConfig::disabled(),
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
async fn login_rate_limit_blocks_repeated_failed_user_attempts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = auth_test_app_with_rate_limit(pool, memory_rate_limit_config());
    let suffix = unique_suffix();
    let body = format!(r#"{{"name":"Nobody {suffix}","password":"wrong"}}"#);

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.clone()))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let second = app
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

    assert_eq!(first.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response_json(second).await["error"]["code"],
        "too_many_attempts"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_request_sends_generic_mail_and_confirm_changes_password() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let suffix = unique_suffix();
    let name = format!("Reset User {suffix}");
    let email = format!("reset-{suffix}@example.test");
    let hash = hash_migrated_input(&legacy_password_input("reset-old")).expect("valid hash");
    let user_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, $2, 'argon2id-md5-v1', 1, $3) RETURNING id",
    )
    .bind(&name)
    .bind(hash)
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("reset user fixture should insert");
    sqlx::query("DELETE FROM password_reset_token WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("reset fixture should clear");
    let (sendmail_path, capture_path) = sendmail_fixture();
    let app = reset_test_app(pool.clone(), sendmail_path);

    let (request_status, request_body) = post_json(
        app.clone(),
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    let message = fs::read_to_string(&capture_path).expect("reset email should be captured");
    let token = reset_token_from_message(&message);
    let stored_token_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM password_reset_token WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("reset token should be readable");
    let (confirm_status, confirm_body) = post_json(
        app.clone(),
        "/api/auth/password-reset/confirm",
        &format!(
            r#"{{"token":"{token}","new_password":"ResetDone2!","confirm_password":"ResetDone2!"}}"#
        ),
    )
    .await;
    let login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"name":"{name}","password":"ResetDone2!"}}"#
                )))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    let used_token_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM password_reset_token WHERE user_id = $1 AND used_at IS NOT NULL",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("used reset token should be readable");

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("reset fixture should be restored");
    if let Some(directory) = capture_path.parent() {
        fs::remove_dir_all(directory).expect("sendmail fixture should be removed");
    }

    assert_eq!(request_status, StatusCode::OK);
    assert_eq!(request_body["requested"], true);
    assert_eq!(
        request_body["message"],
        "If the email exists, a password reset message has been sent."
    );
    assert!(message.contains(&format!("To: {email}")));
    assert!(message.contains("https://forum.example.test/reset_password?token="));
    assert_eq!(stored_token_count, 1);
    assert_eq!(confirm_status, StatusCode::OK);
    assert_eq!(confirm_body["changed"], true);
    assert_eq!(login.status(), StatusCode::OK);
    assert_eq!(used_token_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_request_can_send_mail_through_plain_smtp() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let suffix = unique_suffix();
    let name = format!("SMTP Reset User {suffix}");
    let email = format!("smtp-reset-{suffix}@example.test");
    let hash = hash_migrated_input(&legacy_password_input("reset-old")).expect("valid hash");
    let user_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, $2, 'argon2id-md5-v1', 1, $3) RETURNING id",
    )
    .bind(&name)
    .bind(hash)
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("SMTP reset user fixture should insert");
    sqlx::query("DELETE FROM password_reset_token WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("SMTP reset fixture should clear");
    let (smtp_port, smtp_handle) = smtp_fixture().await;
    let app = reset_test_app_with_config(
        pool.clone(),
        enabled_smtp_reset_config(smtp_port),
        RateLimitConfig::disabled(),
    );

    let (status, body) = post_json(
        app,
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    let message = timeout(Duration::from_secs(3), smtp_handle)
        .await
        .expect("SMTP fixture should receive mail")
        .expect("SMTP fixture should return captured message");
    let stored_token_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM password_reset_token WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("SMTP reset token should be readable");

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("SMTP reset fixture should be restored");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["requested"], true);
    assert!(message.contains(&format!("To: {email}")));
    assert!(message.contains("https://forum.example.test/reset_password?token="));
    assert_eq!(stored_token_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_request_rate_limit_sends_generic_response_without_mail() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let suffix = unique_suffix();
    let email = format!("limited-reset-{suffix}@example.test");
    let user_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, 'fixture', 'argon2id-v1', 1, $2) RETURNING id",
    )
    .bind(format!("Limited Reset {suffix}"))
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("limited reset user fixture should insert");
    let (sendmail_path, capture_path) = sendmail_fixture();
    let app =
        reset_test_app_with_rate_limit(pool.clone(), sendmail_path, memory_rate_limit_config());

    let (first_status, first_body) = post_json(
        app.clone(),
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    assert!(capture_path.exists());
    fs::remove_file(&capture_path).expect("captured first reset mail should be removed");
    let (second_status, second_body) = post_json(
        app,
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    let token_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_token WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("reset tokens should be readable");

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("limited reset user fixture should be removed");
    if let Some(directory) = capture_path.parent() {
        fs::remove_dir_all(directory).expect("sendmail fixture should be removed");
    }

    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first_body["requested"], true);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(second_body["requested"], true);
    assert!(!capture_path.exists());
    assert_eq!(token_rows, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_request_is_generic_for_unknown_or_ambiguous_email() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let suffix = unique_suffix();
    let email = format!("ambiguous-{suffix}@example.test");
    let duplicate_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, 'fixture', 'argon2id-v1', 1, $2) RETURNING id",
    )
    .bind(format!("Duplicate A {suffix}"))
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("duplicate email fixture should insert");
    let duplicate_id_2: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, 'fixture', 'argon2id-v1', 1, $2) RETURNING id",
    )
    .bind(format!("Duplicate B {suffix}"))
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("duplicate email fixture should insert");
    let (sendmail_path, capture_path) = sendmail_fixture();
    let app = reset_test_app(pool.clone(), sendmail_path);

    let (unknown_status, unknown) = post_json(
        app.clone(),
        "/api/auth/password-reset/request",
        r#"{"email":"nobody@example.test"}"#,
    )
    .await;
    let (ambiguous_status, ambiguous) = post_json(
        app,
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    let reset_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_token WHERE user_id IN ($1, $2)")
            .bind(duplicate_id)
            .bind(duplicate_id_2)
            .fetch_one(&pool)
            .await
            .expect("reset table should be readable");

    sqlx::query("DELETE FROM user_info WHERE id IN ($1, $2)")
        .bind(duplicate_id)
        .bind(duplicate_id_2)
        .execute(&pool)
        .await
        .expect("duplicate email fixture should be removed");
    if let Some(directory) = capture_path.parent() {
        fs::remove_dir_all(directory).expect("sendmail fixture should be removed");
    }

    assert_eq!(unknown_status, StatusCode::OK);
    assert_eq!(ambiguous_status, StatusCode::OK);
    assert_eq!(unknown["requested"], true);
    assert_eq!(ambiguous["requested"], true);
    assert!(!capture_path.exists());
    assert_eq!(reset_rows, 0);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_confirm_rejects_invalid_or_expired_tokens() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let suffix = unique_suffix();
    let email = format!("expired-{suffix}@example.test");
    let user_id: i32 = sqlx::query_scalar(
        "INSERT INTO user_info (name, password, password_scheme, level, email) VALUES ($1, 'fixture', 'argon2id-v1', 1, $2) RETURNING id",
    )
    .bind(format!("Expired Reset {suffix}"))
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("expired reset user fixture should insert");
    let (sendmail_path, capture_path) = sendmail_fixture();
    let app = reset_test_app(pool.clone(), sendmail_path);

    let (invalid_status, invalid) = post_json(
        app.clone(),
        "/api/auth/password-reset/confirm",
        r#"{"token":"bad","new_password":"ResetDone2!","confirm_password":"ResetDone2!"}"#,
    )
    .await;
    let _ = post_json(
        app.clone(),
        "/api/auth/password-reset/request",
        &format!(r#"{{"email":"{email}"}}"#),
    )
    .await;
    let token = reset_token_from_message(
        &fs::read_to_string(&capture_path).expect("reset email should be captured"),
    );
    sqlx::query(
        "UPDATE password_reset_token SET expires_at = CURRENT_TIMESTAMP - INTERVAL '1 second' WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("reset token should be expired");
    let (expired_status, expired) = post_json(
        app,
        "/api/auth/password-reset/confirm",
        &format!(
            r#"{{"token":"{token}","new_password":"ResetDone2!","confirm_password":"ResetDone2!"}}"#
        ),
    )
    .await;

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("reset fixture should be restored");
    if let Some(directory) = capture_path.parent() {
        fs::remove_dir_all(directory).expect("sendmail fixture should be removed");
    }

    assert_eq!(invalid_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid["error"]["code"], "invalid_reset_token");
    assert_eq!(expired_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(expired["error"]["code"], "invalid_reset_token");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn password_reset_confirm_rate_limit_blocks_repeated_invalid_tokens() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (sendmail_path, capture_path) = sendmail_fixture();
    let app = reset_test_app_with_rate_limit(pool, sendmail_path, memory_rate_limit_config());

    let (first_status, first_body) = post_json(
        app.clone(),
        "/api/auth/password-reset/confirm",
        r#"{"token":"bad","new_password":"ResetDone2!","confirm_password":"ResetDone2!"}"#,
    )
    .await;
    let (second_status, second_body) = post_json(
        app,
        "/api/auth/password-reset/confirm",
        r#"{"token":"bad","new_password":"ResetDone2!","confirm_password":"ResetDone2!"}"#,
    )
    .await;

    if let Some(directory) = capture_path.parent() {
        fs::remove_dir_all(directory).expect("sendmail fixture should be removed");
    }

    assert_eq!(first_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(first_body["error"]["code"], "invalid_reset_token");
    assert_eq!(second_status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second_body["error"]["code"], "too_many_attempts");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_uses_the_normalized_user_name_index_expression() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let password = "normalized-name-password";
    let hash = hash_migrated_input(&legacy_password_input(password)).expect("valid hash");
    let user_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO user_info (name, password, password_scheme, level)
        VALUES ('  Normalized Login  ', $1, 'argon2id-md5-v1', 1)
        RETURNING id
        "#,
    )
    .bind(hash)
    .fetch_one(&pool)
    .await
    .expect("normalized login fixture should insert");
    let app = common::test_app(pool.clone());

    let (status, body) = post_json(
        app,
        "/api/auth/login",
        &format!(r#"{{"name":"Normalized Login","password":"{password}"}}"#),
    )
    .await;

    sqlx::query("DELETE FROM user_info WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("normalized login fixture should clean up");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
    assert_eq!(body["user"]["name"], "Normalized Login");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn owner_can_change_password_and_is_logged_out() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let activity_before: (Option<String>, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT to_char(last_login, 'YYYY-MM-DD HH24:MI:SS.US'), last_login_ip, login_count FROM user_info WHERE id = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("login activity fixture should be readable");
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
    sqlx::query(
        "UPDATE user_info SET last_login = $1::timestamp, last_login_ip = $2, login_count = $3 WHERE id = 2",
    )
    .bind(activity_before.0)
    .bind(activity_before.1)
    .bind(activity_before.2)
    .execute(&pool)
    .await
    .expect("login activity fixture should be restored");
    assert_eq!(login.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn administrator_can_reset_another_password_without_current_password() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let activity_before: (Option<String>, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT to_char(last_login, 'YYYY-MM-DD HH24:MI:SS.US'), last_login_ip, login_count FROM user_info WHERE id = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("target login activity fixture should be readable");
    let original_credential: (String, Option<String>) =
        sqlx::query_as("SELECT password, password_scheme FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("target credential fixture should be readable");
    let legacy_hash =
        hash_migrated_input(&legacy_password_input("old-carol-password")).expect("valid hash");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = 'argon2id-md5-v1' WHERE id = 3",
    )
    .bind(legacy_hash)
    .execute(&pool)
    .await
    .expect("target legacy credential fixture should update");
    let (app, admin_cookie) = common::authenticated_test_app_as(
        pool.clone(),
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
    let updated_credential: (String, Option<String>) =
        sqlx::query_as("SELECT password, password_scheme FROM user_info WHERE id = 3")
            .fetch_one(&pool)
            .await
            .expect("updated target credential should be readable");
    assert_eq!(
        updated_credential.1.as_deref(),
        Some(MODERN_PASSWORD_SCHEME)
    );
    assert!(dogn3::auth::verify_modern_password(
        "ResetCarol3!",
        &updated_credential.0
    ));
    let admin_session = app
        .clone()
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

    let login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"Carol","password":"ResetCarol3!"}"#))
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    sqlx::query(
        "UPDATE user_info SET password = $1, password_scheme = $2, last_login = $3::timestamp, last_login_ip = $4, login_count = $5 WHERE id = 3",
    )
    .bind(original_credential.0)
    .bind(original_credential.1)
    .bind(activity_before.0)
    .bind(activity_before.1)
    .bind(activity_before.2)
    .execute(&pool)
    .await
    .expect("target login activity fixture should be restored");
    assert_eq!(login.status(), StatusCode::OK);
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
