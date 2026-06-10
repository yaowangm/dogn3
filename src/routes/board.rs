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

const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct BoardQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BoardResponse {
    site_name: String,
    board: BoardInfo,
    pager: Pager,
    recent_announcement_post: Option<BoardPostSummary>,
    trees: Vec<PostTree>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct BoardInfo {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    category_name: String,
    post_count: i32,
    root_count: i32,
    master_users: Vec<BoardMasterSummary>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardMasterSummary {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize)]
pub struct Pager {
    page: i64,
    page_size: i64,
    total_pages: i64,
    total_posts: i64,
    has_previous: bool,
    has_next: bool,
}

#[derive(Debug, Serialize)]
pub struct PostTree {
    root_id: i32,
    posts: Vec<BoardPostSummary>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardPostSummary {
    id: i32,
    root_id: i32,
    parent_id: Option<i32>,
    level: i32,
    subject: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_time: Option<String>,
    size: Option<i32>,
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

#[derive(Debug, FromRow)]
struct BoardInfoRow {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    category_name: String,
    post_count: i32,
    root_count: Option<i32>,
}

pub async fn board(
    Path(board_id): Path<i32>,
    Query(query): Query<BoardQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let can_read_encrypted = auth::is_authenticated(&state, &headers).await?;
    let page_size = query
        .page_size
        .unwrap_or(state.board_page_size)
        .clamp(1, MAX_PAGE_SIZE);
    let page = query.page.unwrap_or(1).max(1);
    let board = board_info(&state, board_id).await?;
    let total_posts = visible_post_count(&state, board_id).await?;
    let total_pages = total_pages(total_posts, page_size);
    let page = if total_pages > 0 {
        page.min(total_pages)
    } else {
        1
    };
    let offset = (page - 1) * page_size;
    let recent_announcement_post =
        recent_announcement_post(&state, board_id, can_read_encrypted).await?;
    let posts = board_posts(&state, board_id, page_size, offset, can_read_encrypted).await?;
    let trees = group_posts_by_tree(posts);
    let boards = board_navigation(&state).await?;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(BoardResponse {
            site_name: state.site_name.clone(),
            board,
            pager: Pager {
                page,
                page_size,
                total_pages,
                total_posts,
                has_previous: page > 1,
                has_next: total_pages > 0 && page < total_pages,
            },
            recent_announcement_post,
            trees,
            boards,
        }),
    )
        .into_response())
}

async fn board_info(state: &AppState, board_id: i32) -> AppResult<BoardInfo> {
    let Some(board) = sqlx::query_as::<_, BoardInfoRow>(
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
        WHERE b.id = $1
        "#,
    )
    .bind(board_id)
    .fetch_optional(&state.pool)
    .await?
    else {
        return Err(AppError::NotFound);
    };
    let master_users = sqlx::query_as::<_, BoardMasterSummary>(
        r#"
        SELECT u.id, BTRIM(u.name) AS name
        FROM board_master bm
        JOIN user_info u ON u.id = bm.user_id
        WHERE bm.board_id = $1
        ORDER BY bm.order_id, u.id
        "#,
    )
    .bind(board_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(BoardInfo {
        id: board.id,
        name: board.name,
        comment: board.comment,
        category_id: board.category_id,
        category_name: board.category_name,
        post_count: board.post_count,
        root_count: board.root_count.unwrap_or(0),
        master_users,
    })
}

async fn visible_post_count(state: &AppState, board_id: i32) -> AppResult<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM post p
        WHERE p.board_id = $1
          AND p.state IN (0, 1)
        "#,
    )
    .bind(board_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(count)
}

async fn board_posts(
    state: &AppState,
    board_id: i32,
    page_size: i64,
    offset: i64,
    can_read_encrypted: bool,
) -> AppResult<Vec<BoardPostSummary>> {
    let posts = sqlx::query_as::<_, BoardPostSummary>(
        r#"
        SELECT
            p.id,
            COALESCE(p.root_id, p.id) AS root_id,
            p.parent_id,
            p.level,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            to_char(p.reply_time, 'YYYY-MM-DD HH24:MI') AS reply_time,
            p.size,
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(BTRIM(p.link_url), '') IS NOT NULL AS has_link,
            NULLIF(BTRIM(p.image_url), '') IS NOT NULL AS has_image,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $4) THEN NULLIF(BTRIM(p.link_url), '') END AS link_url,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $4) THEN NULLIF(BTRIM(p.image_url), '') END AS image_url
        FROM post p
        WHERE p.board_id = $1
          AND p.state IN (0, 1)
        ORDER BY COALESCE(p.root_id, p.id) DESC, p.order_num
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(board_id)
    .bind(page_size)
    .bind(offset)
    .bind(can_read_encrypted)
    .fetch_all(&state.pool)
    .await?;

    Ok(posts)
}

async fn recent_announcement_post(
    state: &AppState,
    board_id: i32,
    can_read_encrypted: bool,
) -> AppResult<Option<BoardPostSummary>> {
    let post = sqlx::query_as::<_, BoardPostSummary>(
        r#"
        SELECT
            p.id,
            COALESCE(p.root_id, p.id) AS root_id,
            p.parent_id,
            p.level,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            to_char(p.reply_time, 'YYYY-MM-DD HH24:MI') AS reply_time,
            p.size,
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
        WHERE p.board_id = $1
          AND p.state IN (0, 1)
          AND p.type = 3
        ORDER BY p.id DESC
        LIMIT 1
        "#,
    )
    .bind(board_id)
    .bind(can_read_encrypted)
    .fetch_optional(&state.pool)
    .await?;

    Ok(post)
}

async fn board_navigation(state: &AppState) -> AppResult<Vec<BoardNavSummary>> {
    let boards = sqlx::query_as::<_, BoardNavSummary>(
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
    .await?;

    Ok(boards)
}

fn total_pages(total_posts: i64, page_size: i64) -> i64 {
    if total_posts == 0 {
        0
    } else {
        (total_posts + page_size - 1) / page_size
    }
}

fn group_posts_by_tree(posts: Vec<BoardPostSummary>) -> Vec<PostTree> {
    let mut trees = Vec::<PostTree>::new();

    for post in posts {
        let root_id = post.root_id;
        match trees.last_mut() {
            Some(tree) if tree.root_id == root_id => tree.posts.push(post),
            _ => trees.push(PostTree {
                root_id,
                posts: vec![post],
            }),
        }
    }

    trees
}
