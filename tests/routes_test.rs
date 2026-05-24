mod common;

use std::{
    fs,
    time::{Duration, SystemTime},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dogn3::{
    auth::AuthenticatedUser,
    build_router,
    state::{AppState, AuthRuntimeConfig},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn configured_image_directory_serves_post_images() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory =
        std::env::temp_dir().join(format!("dogn3-test-images-{}-{unique}", std::process::id()));
    let image_path = image_directory.join("pic/200809/sample.JPG");
    let denied_path = image_directory.join("pic/200809/info.php");
    fs::create_dir_all(image_path.parent().expect("image parent")).expect("create image fixture");
    fs::write(&image_path, b"test-image").expect("write image fixture");
    fs::write(&denied_path, b"<?php echo 'private';").expect("write denied fixture");

    let pool = PgPoolOptions::new()
        .connect_lazy("postgres:///dogn_test")
        .expect("valid lazy PostgreSQL pool");
    let app = build_router(AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        image_directory.clone(),
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    ));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/pic/200809/sample.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "image/jpeg",
        "image type should be constrained by approved extension"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = response
        .into_body()
        .collect()
        .await
        .expect("image body should be readable")
        .to_bytes();
    assert_eq!(body.as_ref(), b"test-image");

    let denied_response = app
        .oneshot(
            Request::builder()
                .uri("/images/pic/200809/info.php")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("denied image route should respond");
    assert_eq!(denied_response.status(), StatusCode::NOT_FOUND);

    fs::remove_dir_all(image_directory).expect("clean image fixture");
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn configured_image_directory_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let fixture_directory = std::env::temp_dir().join(format!(
        "dogn3-test-symlink-{}-{unique}",
        std::process::id()
    ));
    let image_directory = fixture_directory.join("images");
    let external_image = fixture_directory.join("outside.JPG");
    let linked_image = image_directory.join("linked.JPG");
    fs::create_dir_all(&image_directory).expect("create image directory");
    fs::write(&external_image, b"external-image").expect("write external fixture");
    symlink(&external_image, &linked_image).expect("create symlink fixture");

    let pool = PgPoolOptions::new()
        .connect_lazy("postgres:///dogn_test")
        .expect("valid lazy PostgreSQL pool");
    let app = build_router(AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        image_directory,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
    ));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/images/linked.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    fs::remove_dir_all(fixture_directory).expect("clean image fixture");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn encrypted_post_image_requires_login() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory = std::env::temp_dir().join(format!(
        "dogn3-private-images-{}-{unique}",
        std::process::id()
    ));
    let image_path = image_directory.join("pic/private.JPG");
    let unknown_image_path = image_directory.join("pic/unknown.JPG");
    fs::create_dir_all(image_path.parent().expect("image parent")).expect("create image fixture");
    fs::write(&image_path, b"private-image").expect("write image fixture");
    fs::write(&unknown_image_path, b"unknown-image").expect("write unknown image fixture");

    let state = AppState::new(
        pool,
        None,
        "Test Forum".to_string(),
        50,
        image_directory.clone(),
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
    let app = build_router(state);

    let public = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/pic/private.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(public.status(), StatusCode::NOT_FOUND);
    assert_eq!(public.headers()["cache-control"], "no-store");

    let authenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/pic/private.JPG")
                .header("cookie", format!("dogn_session={token}"))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(authenticated.status(), StatusCode::OK);
    assert_eq!(authenticated.headers()["cache-control"], "no-store");

    let unknown = app
        .oneshot(
            Request::builder()
                .uri("/images/pic/unknown.JPG")
                .header("cookie", format!("dogn_session={token}"))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert_eq!(unknown.headers()["cache-control"], "no-store");

    fs::remove_dir_all(image_directory).expect("clean image fixture");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn index_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body = String::from_utf8(body.to_vec()).expect("body should be utf-8");

    assert!(body.contains("<!doctype html>"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn login_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn board_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/board/11")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body = String::from_utf8(body.to_vec()).expect("body should be utf-8");

    assert!(body.contains("<dogn-app-shell>"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/user/2")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post/101")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_list_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post_list/101")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_print_page_returns_minimal_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post_print/101")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body = String::from_utf8(body.to_vec()).expect("body should be utf-8");

    assert!(body.contains("data-page-mode=\"print\""));
    assert!(!body.contains("class=\"topbar\""));
    assert!(!body.contains("class=\"footer\""));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn health_endpoint_reports_database_ok() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["cache"], "disabled");
}
