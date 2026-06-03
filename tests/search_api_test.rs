mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

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
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("response should be json");
    (status, body)
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_search_requires_login() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let app = common::test_app(pool);

    let (status, body) = get_json_with_cookie(app, "/api/search/posts", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "authentication_required");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_search_filters_and_orders_visible_posts() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);

    let (status, body) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?subject=Original&order=id_asc&page_size=2",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["order"], "id_asc");
    assert_eq!(body["filters"]["subject"], "Original");
    assert_eq!(body["pager"]["total_posts"], 2);
    assert_eq!(body["posts"][0]["id"], 101);
    assert_eq!(body["posts"][1]["id"], 102);

    let (status, image) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?has_image=true",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(image["pager"]["total_posts"], 2);
    assert!(
        image["posts"]
            .as_array()
            .expect("posts")
            .iter()
            .all(|post| {
                post["has_image"].as_bool().unwrap_or(false)
                    && post["state"].as_i64().unwrap_or_default() != 2
            })
    );

    let (status, deleted) =
        get_json_with_cookie(app, "/api/search/posts?subject=Deleted", Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["pager"]["total_posts"], 0);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_search_filters_content_user_dates_type_and_links() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let (app, cookie) = common::authenticated_test_app(pool);

    let (status, content) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?content=paragraph&user_name=Bob&has_link=true&post_type=1",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content["pager"]["total_posts"], 1);
    assert_eq!(content["posts"][0]["id"], 101);
    assert!(content["posts"][0].get("content_excerpt").is_none());

    let (status, dates) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?created_from=2024-02-03&created_to=2024-02-03&replied_from=2024-02-03&replied_to=2024-02-03",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dates["pager"]["total_posts"], 1);
    assert_eq!(dates["posts"][0]["id"], 103);

    let (status, invalid_date) = get_json_with_cookie(
        app,
        "/api/search/posts?created_from=2024-02-31",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_date["error"]["code"], "invalid_search_filter");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; use ./scripts/test.sh"]
async fn post_search_supports_chinese_substring_matching() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    sqlx::query(
        r#"
        INSERT INTO post (
            id, subject, board_id, user_id, user_name, post_time, reply_time, size,
            reply_count, access_count, point, type, state, content, parent_id, root_id,
            level, order_num
        )
        VALUES (
            900, '中文主题搜索', 11, 2, '中文用户', '2024-02-06 09:00:00',
            '2024-02-06 09:00:00', 32, 0, 1, 0, 0, 0, '这里包含中文内容关键词',
            0, 900, 0, 0
        )
        ON CONFLICT (id) DO UPDATE
        SET subject = EXCLUDED.subject,
            content = EXCLUDED.content,
            user_name = EXCLUDED.user_name,
            state = EXCLUDED.state
        "#,
    )
    .execute(&pool)
    .await
    .expect("Chinese search fixture should be written");
    let (app, cookie) = common::authenticated_test_app(pool.clone());

    let (status, subject) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?subject=%E4%B8%BB%E9%A2%98%E6%90%9C%E7%B4%A2",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(subject["pager"]["total_posts"], 1);
    assert_eq!(subject["posts"][0]["id"], 900);

    let (status, content) = get_json_with_cookie(
        app.clone(),
        "/api/search/posts?content=%E4%B8%AD%E6%96%87%E5%86%85%E5%AE%B9",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content["pager"]["total_posts"], 1);
    assert_eq!(content["posts"][0]["id"], 900);

    let (status, user) = get_json_with_cookie(
        app,
        "/api/search/posts?user_name=%E4%B8%AD%E6%96%87%E7%94%A8%E6%88%B7",
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["pager"]["total_posts"], 1);
    assert_eq!(user["posts"][0]["id"], 900);

    sqlx::query("DELETE FROM post WHERE id = 900")
        .execute(&pool)
        .await
        .expect("Chinese search fixture should be removed");
}
