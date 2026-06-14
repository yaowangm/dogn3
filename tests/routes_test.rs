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
    rate_limit::RateLimitConfig,
    state::{AppState, AuthRuntimeConfig},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

async fn response_text(response: axum::response::Response) -> String {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    String::from_utf8(body.to_vec()).expect("body should be utf-8")
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn configured_image_directory_serves_post_images() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let image_directory =
        std::env::temp_dir().join(format!("dogn3-test-images-{}-{unique}", std::process::id()));
    let image_path = image_directory.join("200809/sample.JPG");
    let legacy_prefixed_image_path = image_directory.join("200811/prefixed.JPG");
    let denied_path = image_directory.join("200809/info.php");
    let orphaned_upload_path = image_directory.join("uploads/post-999.jpg");
    let orphaned_month_path = image_directory.join("202606/random.JPG");
    let orphaned_legacy_prefixed_path = image_directory.join("202607/orphan.JPG");
    fs::create_dir_all(image_path.parent().expect("image parent")).expect("create image fixture");
    fs::create_dir_all(
        legacy_prefixed_image_path
            .parent()
            .expect("legacy prefixed image parent"),
    )
    .expect("create legacy prefixed image fixture");
    fs::create_dir_all(orphaned_upload_path.parent().expect("upload parent"))
        .expect("create upload fixture");
    fs::create_dir_all(orphaned_month_path.parent().expect("month parent"))
        .expect("create month fixture");
    fs::create_dir_all(
        orphaned_legacy_prefixed_path
            .parent()
            .expect("orphaned legacy prefixed image parent"),
    )
    .expect("create orphaned legacy prefixed image fixture");
    fs::write(&image_path, b"test-image").expect("write image fixture");
    fs::write(&legacy_prefixed_image_path, b"legacy-prefixed-image")
        .expect("write legacy prefixed image fixture");
    fs::write(&denied_path, b"<?php echo 'private';").expect("write denied fixture");
    fs::write(&orphaned_upload_path, b"orphaned-upload").expect("write upload fixture");
    fs::write(&orphaned_month_path, b"orphaned-month").expect("write month fixture");
    fs::write(&orphaned_legacy_prefixed_path, b"orphaned-legacy-prefixed")
        .expect("write orphaned legacy prefixed fixture");

    let Some(pool) = common::test_pool().await else {
        return;
    };
    let original_image_url: Option<String> =
        sqlx::query_scalar("SELECT image_url FROM post WHERE id = 100")
            .fetch_one(&pool)
            .await
            .expect("post image fixture should load");
    sqlx::query("UPDATE post SET image_url = '200809/sample.JPG' WHERE id = 100")
        .execute(&pool)
        .await
        .expect("post image fixture should be prepared");

    let app = build_router(AppState::new(
        pool.clone(),
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
        image_directory.clone(),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
        common::disabled_password_reset_config(),
        RateLimitConfig::disabled(),
    ));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/200809/sample.JPG")
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

    sqlx::query("UPDATE post SET image_url = 'pic/200811/prefixed.JPG' WHERE id = 100")
        .execute(&pool)
        .await
        .expect("legacy prefixed image fixture should be prepared");
    let legacy_prefixed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/pic/200811/prefixed.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("legacy prefixed image route should respond");
    assert_eq!(legacy_prefixed_response.status(), StatusCode::OK);
    let legacy_prefixed_body = legacy_prefixed_response
        .into_body()
        .collect()
        .await
        .expect("legacy prefixed image body should be readable")
        .to_bytes();
    assert_eq!(legacy_prefixed_body.as_ref(), b"legacy-prefixed-image");

    let denied_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/200809/info.php")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("denied image route should respond");
    assert_eq!(denied_response.status(), StatusCode::NOT_FOUND);

    let orphaned_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/uploads/post-999.jpg")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("orphaned upload route should respond");
    assert_eq!(orphaned_response.status(), StatusCode::NOT_FOUND);

    let orphaned_month_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/images/202606/random.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("orphaned monthly image route should respond");
    assert_eq!(orphaned_month_response.status(), StatusCode::NOT_FOUND);

    let orphaned_legacy_prefixed_response = app
        .oneshot(
            Request::builder()
                .uri("/images/pic/202607/orphan.JPG")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("orphaned legacy prefixed image route should respond");
    assert_eq!(
        orphaned_legacy_prefixed_response.status(),
        StatusCode::NOT_FOUND
    );

    sqlx::query("UPDATE post SET image_url = $1 WHERE id = 100")
        .bind(original_image_url)
        .execute(&pool)
        .await
        .expect("post image fixture should be restored");
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
        10,
        100,
        100,
        2,
        5,
        10,
        50,
        131_072,
        1_000,
        image_directory,
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
        common::disabled_password_reset_config(),
        RateLimitConfig::disabled(),
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
    let image_path = image_directory.join("private.JPG");
    let unknown_image_path = image_directory.join("unknown.JPG");
    fs::create_dir_all(image_path.parent().expect("image parent")).expect("create image fixture");
    fs::write(&image_path, b"private-image").expect("write image fixture");
    fs::write(&unknown_image_path, b"unknown-image").expect("write unknown image fixture");

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
        image_directory.clone(),
        2_097_152,
        AuthRuntimeConfig {
            session_ttl: Duration::from_secs(3600),
            session_cookie_secure: false,
            login_max_concurrent_hashes: 2,
        },
        common::disabled_password_reset_config(),
        RateLimitConfig::disabled(),
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
                .uri("/images/private.JPG")
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
                .uri("/images/private.JPG")
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
                .uri("/images/unknown.JPG")
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
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_text(response).await;

    assert!(body.contains("<!doctype html>"));
    assert!(body.contains(r#"<meta property="og:type" content="website">"#));
    assert!(body.contains(r#"<meta property="og:title" content="Test Forum">"#));
    assert!(body.contains(r#"<meta property="og:image" content="/assets/share.png?v="#));
    assert!(body.contains(r#"<meta property="og:image:type" content="image/png">"#));
    assert!(body.contains(r#"<meta property="og:image:width" content="512">"#));
    assert!(body.contains(r#"<meta property="og:image:height" content="512">"#));
    assert!(!body.contains("cdn.jsdelivr.net/npm/katex"));
    assert!(!body.contains("Recent root posts"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn versioned_katex_assets_are_served_with_immutable_caching() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/vendor/katex-0.16.22/katex.min.js")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
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
async fn reset_password_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/reset_password?token=fixture")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn versioned_application_assets_are_served_with_immutable_caching() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/js/app.js?v=test-version")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn html_shell_revalidates_and_references_build_versioned_assets() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-cache"
    );
    let etag = response.headers()[axum::http::header::ETAG].clone();
    let body = response_text(response).await;
    assert!(!body.contains("{{ASSET_VERSION}}"));
    assert!(body.contains(r#"/assets/css/app.css?v="#));
    assert!(body.contains(r#"/assets/favicon.svg?v="#));
    assert!(body.contains(r#"/assets/js/i18n.js?v="#));
    assert!(body.contains(r#"/assets/js/app.js?v="#));

    let revalidated = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(axum::http::header::IF_NONE_MATCH, etag)
                .body(Body::empty())
                .expect("valid conditional request"),
        )
        .await
        .expect("route should revalidate");
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
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

    let body = response_text(response).await;

    assert!(body.contains("<dogn-app-shell>"));
    assert!(body.contains(r#"<title>Chat - Test Forum</title>"#));
    assert!(body.contains(r#"<meta property="og:title" content="Chat - Test Forum">"#));
    assert!(body.contains(r#"<meta property="og:description" content="General discussion">"#));
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

    let body = response_text(response).await;
    assert!(body.contains(r#"<title>Bob - Test Forum</title>"#));
    assert!(body.contains(r#"<meta property="og:type" content="profile">"#));
    assert!(body.contains(r#"<meta property="og:description" content="Rust reader.">"#));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_list_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/user_list")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn chinese_ui_translations_cover_generated_user_and_post_labels() {
    let translations = include_str!("../static/js/i18n.js");
    let application = include_str!("../static/js/app.js");

    assert!(translations.contains(r#""originals": "原创""#));
    assert!(translations.contains(r#""Encrypted": "已加密""#));
    assert!(application.contains(r#"uiText("Encrypted")"#));
    assert!(application.contains("escapeHtml(uiText(label))"));

    let user_page_pattern = translations
        .find(r#".replace(/^Page (.+) \/ (.+) \((.+) users\)$/"#)
        .expect("user-count pager translation should exist");
    let post_page_pattern = translations
        .find(r#".replace(/^Page (.+) \/ (.+) \((.+) posts\)$/"#)
        .expect("post-count pager translation should exist");
    let generic_page_pattern = translations
        .find(r#".replace(/^Page (.+) \/ (.+)$/"#)
        .expect("generic pager translation should exist");
    assert!(user_page_pattern < generic_page_pattern);
    assert!(post_page_pattern < generic_page_pattern);

    let post_card = application
        .split("  renderPostCard(post) {")
        .nth(1)
        .expect("post-card renderer should exist");
    let size_position = post_card
        .find("postMetaIcons.size")
        .expect("post-card size metadata should exist");
    let replies_position = post_card
        .find("postMetaIcons.replies")
        .expect("post-card reply metadata should exist");
    assert!(size_position < replies_position);
    assert!(application.contains(r#"<span data-no-i18n>${escapeHtml(value)}</span>"#));
    assert!(application.contains("prefetchJson(initialDataPath)"));
    assert!(!application.contains(r#"cache: "no-store""#));
    assert!(application.contains("window.requestAnimationFrame(updatePreview)"));
    assert!(application.contains("const boardsByCategory = new Map()"));
    assert!(application.contains(r#"root.dataset.siteManagerActionsBound = "true""#));
    assert!(application.contains(r#"root.addEventListener("submit""#));
    assert!(application.contains(r#"root.addEventListener("click""#));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn encrypted_post_page_meta_does_not_expose_content() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post/103")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_text(response).await;
    assert!(body.contains(r#"<title>Forward root - Test Forum</title>"#));
    assert!(body.contains(
        r#"<meta property="og:description" content="Encrypted post metadata for Forward root.">"#
    ));
    assert!(!body.contains("Encrypted body."));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn search_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn user_add_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/user_add")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn site_manager_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/site_mgr")
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

    let body = response_text(response).await;
    assert!(body.contains(r#"<title>Original root - Test Forum</title>"#));
    assert!(body.contains(r#"<meta property="og:type" content="article">"#));
    assert!(body.contains(
        r#"<meta property="og:description" content="A full original post. Second paragraph.">"#
    ));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_update_page_returns_html_shell() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/post_upd?board_id=11")
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
