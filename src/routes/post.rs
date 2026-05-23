use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use sqlx::FromRow;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct PostResponse {
    site_name: String,
    post: PostDetail,
    board: PostBoard,
    tree: PostTree,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct PostBoard {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize)]
pub struct PostTree {
    root_id: i32,
    posts: Vec<TreePostSummary>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TreePostSummary {
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
    link_url: Option<String>,
    image_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostDetail {
    id: i32,
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
    content: Option<String>,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    signature: Option<SignatureSummary>,
    point_awards: Vec<PointAward>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SignatureSummary {
    id: i32,
    content: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PointAward {
    user_id: i32,
    user_name: Option<String>,
    point: i32,
}

#[derive(Debug, FromRow)]
struct PostDetailRow {
    id: i32,
    board_id: i32,
    board_name: String,
    root_id: i32,
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
    content: Option<String>,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    sign_id: Option<i32>,
}

pub async fn post(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
) -> AppResult<Json<PostResponse>> {
    let row = post_detail(&state, post_id).await?;
    let signature = match row.sign_id {
        Some(sign_id) => signature(&state, sign_id).await?,
        None => None,
    };
    let point_awards = if row.point.unwrap_or(0) != 0 {
        point_awards(&state, row.id).await?
    } else {
        Vec::new()
    };
    let tree = post_tree(&state, row.root_id).await?;
    let boards = board_navigation(&state).await?;

    Ok(Json(PostResponse {
        site_name: state.site_name.clone(),
        board: PostBoard {
            id: row.board_id,
            name: row.board_name,
        },
        tree,
        boards,
        post: PostDetail {
            id: row.id,
            subject: row.subject,
            user_id: row.user_id,
            user_name: row.user_name,
            post_time: row.post_time,
            reply_time: row.reply_time,
            size: row.size,
            reply_count: row.reply_count,
            access_count: row.access_count,
            point: row.point,
            post_type: row.post_type,
            state: row.state,
            content: row.content,
            link_name: row.link_name,
            link_url: row.link_url,
            image_url: row.image_url,
            signature,
            point_awards,
        },
    }))
}

async fn post_detail(state: &AppState, post_id: i32) -> AppResult<PostDetailRow> {
    let row = sqlx::query_as::<_, PostDetailRow>(
        r#"
        SELECT
            p.id,
            p.board_id,
            BTRIM(b.name) AS board_name,
            COALESCE(p.root_id, p.id) AS root_id,
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
            NULLIF(p.content, '') AS content,
            NULLIF(BTRIM(p.link_name), '') AS link_name,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url,
            p.sign_id
        FROM post p
        JOIN board b ON b.id = p.board_id
        WHERE p.id = $1
          AND p.state <> 2
        "#,
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await?;

    row.ok_or(AppError::NotFound)
}

async fn signature(state: &AppState, sign_id: i32) -> AppResult<Option<SignatureSummary>> {
    let signature = sqlx::query_as::<_, SignatureSummary>(
        r#"
        SELECT id, NULLIF(content, '') AS content
        FROM post
        WHERE id = $1
          AND state = 0
        "#,
    )
    .bind(sign_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(signature)
}

async fn point_awards(state: &AppState, post_id: i32) -> AppResult<Vec<PointAward>> {
    let awards = sqlx::query_as::<_, PointAward>(
        r#"
        SELECT
            pl.user_id,
            NULLIF(BTRIM(u.name), '') AS user_name,
            pl.point
        FROM point_log pl
        LEFT JOIN user_info u ON u.id = pl.user_id
        WHERE pl.post_id = $1
        ORDER BY pl.post_time, pl.id
        "#,
    )
    .bind(post_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(awards)
}

async fn post_tree(state: &AppState, root_id: i32) -> AppResult<PostTree> {
    let posts = sqlx::query_as::<_, TreePostSummary>(
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
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url
        FROM post p
        WHERE COALESCE(p.root_id, p.id) = $1
          AND p.state <> 2
        ORDER BY p.order_num
        "#,
    )
    .bind(root_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(PostTree { root_id, posts })
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
