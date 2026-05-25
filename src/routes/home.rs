use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::AppResult, routes::auth, state::AppState};

const PUBLIC_HOME_CACHE_KEY: &str = "api:home:v3:public";
const AUTHENTICATED_HOME_CACHE_KEY: &str = "api:home:v3:authenticated";

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
    has_link: bool,
    has_image: bool,
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

pub async fn home(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let can_read_encrypted = auth::is_authenticated(&state, &headers);
    let cache_key = if can_read_encrypted {
        AUTHENTICATED_HOME_CACHE_KEY
    } else {
        PUBLIC_HOME_CACHE_KEY
    };

    if let Some(cache) = &state.cache {
        match cache.get_json::<HomeResponse>(cache_key).await {
            Ok(Some(response)) => return Ok(no_store_json(response)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = ?error, cache_key, "failed to read home cache");
            }
        }
    }

    let response = build_home_response(&state, can_read_encrypted).await?;

    if let Some(cache) = &state.cache
        && let Err(error) = cache.set_json(cache_key, &response).await
    {
        tracing::warn!(error = ?error, cache_key, "failed to write home cache");
    }

    Ok(no_store_json(response))
}

pub(super) async fn invalidate_cache(state: &AppState) {
    let Some(cache) = &state.cache else {
        return;
    };

    for cache_key in [PUBLIC_HOME_CACHE_KEY, AUTHENTICATED_HOME_CACHE_KEY] {
        if let Err(error) = cache.delete(cache_key).await {
            tracing::warn!(error = ?error, cache_key, "failed to invalidate home cache");
        }
    }
}

fn no_store_json(body: HomeResponse) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

async fn build_home_response(
    state: &AppState,
    can_read_encrypted: bool,
) -> AppResult<HomeResponse> {
    let recent_announcement_posts = posts_by_type(state, 3, can_read_encrypted).await?;
    let recent_root_posts = root_posts(state, can_read_encrypted).await?;
    let recent_original_posts = posts_by_type(state, 1, can_read_encrypted).await?;
    let recent_forward_posts = posts_by_type(state, 2, can_read_encrypted).await?;
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

async fn root_posts(state: &AppState, can_read_encrypted: bool) -> AppResult<Vec<PostSummary>> {
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
            NULLIF(BTRIM(p.link_url), '') IS NOT NULL AS has_link,
            NULLIF(BTRIM(p.image_url), '') IS NOT NULL AS has_image,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $1) THEN NULLIF(BTRIM(p.link_url), '') END AS link_url,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $1) THEN NULLIF(BTRIM(p.image_url), '') END AS image_url
        FROM post p
        LEFT JOIN board b ON b.id = p.board_id
        WHERE COALESCE(p.parent_id, 0) = 0
          AND p.state IN (0, 1)
        ORDER BY p.root_id DESC NULLS LAST, p.order_num
        LIMIT 10
        "#,
    )
    .bind(can_read_encrypted)
    .fetch_all(&state.pool)
    .await?;

    Ok(posts)
}

async fn posts_by_type(
    state: &AppState,
    post_type: i32,
    can_read_encrypted: bool,
) -> AppResult<Vec<PostSummary>> {
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
            NULLIF(BTRIM(p.link_url), '') IS NOT NULL AS has_link,
            NULLIF(BTRIM(p.image_url), '') IS NOT NULL AS has_image,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $2) THEN NULLIF(BTRIM(p.link_url), '') END AS link_url,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $2) THEN NULLIF(BTRIM(p.image_url), '') END AS image_url
        FROM post p
        LEFT JOIN board b ON b.id = p.board_id
        WHERE p.type = $1
          AND p.state IN (0, 1)
        ORDER BY p.id DESC
        LIMIT 10
        "#,
    )
    .bind(post_type)
    .bind(can_read_encrypted)
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
