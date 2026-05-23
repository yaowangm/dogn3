use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::AppResult, state::AppState};

const HOME_CACHE_KEY: &str = "api:home:v2";

#[derive(Debug, Deserialize, Serialize)]
pub struct HomeResponse {
    site_name: String,
    recent_announcement_posts: Vec<PostSummary>,
    recent_root_posts: Vec<PostSummary>,
    recent_original_posts: Vec<PostSummary>,
    recent_forward_posts: Vec<PostSummary>,
    new_users: Vec<UserSummary>,
    top_point_users: Vec<UserSummary>,
    boards: Vec<BoardSummary>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct PostSummary {
    id: i32,
    subject: Option<String>,
    board_id: Option<i32>,
    board_name: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_count: Option<i32>,
    access_count: i32,
    point: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    link_url: Option<String>,
    image_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct UserSummary {
    id: i32,
    name: String,
    reg_time: Option<String>,
    post_count: i32,
    point: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct BoardSummary {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    category_name: String,
    post_count: i32,
    root_count: Option<i32>,
}

pub async fn home(State(state): State<AppState>) -> AppResult<Json<HomeResponse>> {
    if let Some(cache) = &state.cache {
        match cache.get_json::<HomeResponse>(HOME_CACHE_KEY).await {
            Ok(Some(response)) => return Ok(Json(response)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = ?error, cache_key = HOME_CACHE_KEY, "failed to read home cache");
            }
        }
    }

    let response = build_home_response(&state).await?;

    if let Some(cache) = &state.cache
        && let Err(error) = cache.set_json(HOME_CACHE_KEY, &response).await
    {
        tracing::warn!(error = ?error, cache_key = HOME_CACHE_KEY, "failed to write home cache");
    }

    Ok(Json(response))
}

async fn build_home_response(state: &AppState) -> AppResult<HomeResponse> {
    let recent_announcement_posts = posts_by_type(state, 3).await?;
    let recent_root_posts = root_posts(state).await?;
    let recent_original_posts = posts_by_type(state, 1).await?;
    let recent_forward_posts = posts_by_type(state, 2).await?;
    let new_users = new_users(state).await?;
    let top_point_users = top_point_users(state).await?;
    let boards = boards(state).await?;

    Ok(HomeResponse {
        site_name: state.site_name.clone(),
        recent_announcement_posts,
        recent_root_posts,
        recent_original_posts,
        recent_forward_posts,
        new_users,
        top_point_users,
        boards,
    })
}

async fn root_posts(state: &AppState) -> AppResult<Vec<PostSummary>> {
    let posts = sqlx::query_as::<_, PostSummary>(
        r#"
        SELECT
            p.id,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.board_id,
            NULLIF(BTRIM(b.name), '') AS board_name,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url
        FROM post p
        LEFT JOIN board b ON b.id = p.board_id
        WHERE COALESCE(p.parent_id, 0) = 0
          AND p.state <> 2
        ORDER BY p.root_id DESC NULLS LAST, p.order_num
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(posts)
}

async fn posts_by_type(state: &AppState, post_type: i32) -> AppResult<Vec<PostSummary>> {
    let posts = sqlx::query_as::<_, PostSummary>(
        r#"
        SELECT
            p.id,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.board_id,
            NULLIF(BTRIM(b.name), '') AS board_name,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url
        FROM post p
        LEFT JOIN board b ON b.id = p.board_id
        WHERE p.type = $1
          AND p.state <> 2
        ORDER BY p.id DESC
        LIMIT 10
        "#,
    )
    .bind(post_type)
    .fetch_all(&state.pool)
    .await?;

    Ok(posts)
}

async fn new_users(state: &AppState) -> AppResult<Vec<UserSummary>> {
    let users = sqlx::query_as::<_, UserSummary>(
        r#"
        SELECT
            id,
            BTRIM(name) AS name,
            to_char(reg_time, 'YYYY-MM-DD') AS reg_time,
            post_count,
            point
        FROM user_info
        ORDER BY id DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(users)
}

async fn top_point_users(state: &AppState) -> AppResult<Vec<UserSummary>> {
    let users = sqlx::query_as::<_, UserSummary>(
        r#"
        SELECT
            id,
            BTRIM(name) AS name,
            to_char(reg_time, 'YYYY-MM-DD') AS reg_time,
            post_count,
            point
        FROM user_info
        ORDER BY point DESC NULLS LAST, id ASC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(users)
}

async fn boards(state: &AppState) -> AppResult<Vec<BoardSummary>> {
    let boards = sqlx::query_as::<_, BoardSummary>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            NULLIF(BTRIM(b.comment), '') AS comment,
            b.category_id,
            BTRIM(c.name) AS category_name,
            b.post_count,
            b.root_count
        FROM board b
        JOIN category c ON c.id = b.category_id
        ORDER BY c.order_id, b.order_id, b.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(boards)
}
