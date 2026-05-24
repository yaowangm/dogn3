use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    error::{AppError, AppResult},
    routes::auth,
    state::AppState,
};

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 100;
const ADMIN_LEVEL: i32 = 10;

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    activity: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActivityKind {
    Original,
    Favorites,
    Signatures,
}

impl ActivityKind {
    fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("favorites") => Self::Favorites,
            Some("signatures") => Self::Signatures,
            _ => Self::Original,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    site_name: String,
    user: UserProfile,
    can_update: bool,
    activity: ActivityKind,
    pager: Pager,
    posts: Vec<ActivityPost>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize, FromRow)]
struct UserProfile {
    id: i32,
    name: String,
    level: i32,
    reg_time: Option<String>,
    post_count: i32,
    point: Option<i32>,
    intro: Option<String>,
    favorite_count: Option<i32>,
}

#[derive(Debug, Serialize)]
struct Pager {
    page: i64,
    page_size: i64,
    total_pages: i64,
    total_posts: i64,
    has_previous: bool,
    has_next: bool,
}

#[derive(Debug, Serialize, FromRow)]
struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ActivityPost {
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

pub async fn user(
    Path(user_id): Path<i32>,
    Query(query): Query<UserQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let activity = ActivityKind::from_query(query.activity.as_deref());
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let requested_page = query.page.unwrap_or(1).max(1);
    let user = user_profile(&state, user_id).await?;
    let viewer = auth::current_user(&state, &headers);
    let can_read_encrypted = viewer.is_some();
    let can_update = viewer
        .as_ref()
        .is_some_and(|viewer| viewer.id == user_id || viewer.level >= ADMIN_LEVEL);
    let total_posts = activity_count(&state, user_id, activity).await?;
    let total_pages = total_pages(total_posts, page_size);
    let page = requested_page.min(total_pages.max(1));
    let posts = activity_posts(
        &state,
        user_id,
        activity,
        page_size,
        (page - 1) * page_size,
        can_read_encrypted,
    )
    .await?;
    let boards = board_navigation(&state).await?;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(UserResponse {
            site_name: state.site_name.clone(),
            user,
            can_update,
            activity,
            pager: Pager {
                page,
                page_size,
                total_pages,
                total_posts,
                has_previous: page > 1,
                has_next: total_pages > 0 && page < total_pages,
            },
            posts,
            boards,
        }),
    )
        .into_response())
}

async fn user_profile(state: &AppState, user_id: i32) -> AppResult<UserProfile> {
    sqlx::query_as::<_, UserProfile>(
        r#"
        SELECT
            id,
            BTRIM(name) AS name,
            level,
            to_char(reg_time, 'YYYY-MM-DD') AS reg_time,
            post_count,
            point,
            NULLIF(BTRIM(intro), '') AS intro,
            favorite_count
        FROM user_info
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn activity_count(state: &AppState, user_id: i32, activity: ActivityKind) -> AppResult<i64> {
    let query = match activity {
        ActivityKind::Original => {
            "SELECT COUNT(*) FROM post p WHERE p.user_id = $1 AND p.type = 1 AND p.state IN (0, 1)"
        }
        ActivityKind::Favorites => {
            "SELECT COUNT(*) FROM favorite f JOIN post p ON p.id = f.post_id WHERE f.user_id = $1 AND p.state IN (0, 1)"
        }
        ActivityKind::Signatures => {
            "SELECT COUNT(*) FROM sign_log s JOIN post p ON p.id = s.sign_id WHERE s.user_id = $1 AND p.state IN (0, 1)"
        }
    };

    Ok(sqlx::query_scalar(query)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?)
}

async fn activity_posts(
    state: &AppState,
    user_id: i32,
    activity: ActivityKind,
    page_size: i64,
    offset: i64,
    can_read_encrypted: bool,
) -> AppResult<Vec<ActivityPost>> {
    let source = match activity {
        ActivityKind::Original => "post p",
        ActivityKind::Favorites => "favorite activity JOIN post p ON p.id = activity.post_id",
        ActivityKind::Signatures => "sign_log activity JOIN post p ON p.id = activity.sign_id",
    };
    let filter = match activity {
        ActivityKind::Original => "p.user_id = $1 AND p.type = 1",
        ActivityKind::Favorites | ActivityKind::Signatures => "activity.user_id = $1",
    };
    let query = format!(
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
            CASE WHEN p.state = 0 OR (p.state = 1 AND $4) THEN NULLIF(BTRIM(p.link_url), '') END AS link_url,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $4) THEN NULLIF(BTRIM(p.image_url), '') END AS image_url
        FROM {source}
        LEFT JOIN board b ON b.id = p.board_id
        WHERE {filter}
          AND p.state IN (0, 1)
        ORDER BY p.id DESC
        LIMIT $2 OFFSET $3
        "#
    );

    Ok(sqlx::query_as::<_, ActivityPost>(&query)
        .bind(user_id)
        .bind(page_size)
        .bind(offset)
        .bind(can_read_encrypted)
        .fetch_all(&state.pool)
        .await?)
}

async fn board_navigation(state: &AppState) -> AppResult<Vec<BoardNavSummary>> {
    Ok(sqlx::query_as::<_, BoardNavSummary>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            b.category_id,
            BTRIM(c.name) AS category_name
        FROM board b
        JOIN category c ON c.id = b.category_id
        ORDER BY c.order_id, b.order_id, b.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?)
}

fn total_pages(total_posts: i64, page_size: i64) -> i64 {
    if total_posts == 0 {
        0
    } else {
        (total_posts + page_size - 1) / page_size
    }
}
