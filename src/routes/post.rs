use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use sqlx::FromRow;

use crate::{
    error::{AppError, AppResult},
    routes::auth,
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
pub struct PostListResponse {
    site_name: String,
    selected_post_id: i32,
    board: PostBoard,
    posts: Vec<PostDetail>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct PostPrintResponse {
    site_name: String,
    post: PostDetail,
    board: PostBoard,
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
    has_link: bool,
    has_image: bool,
    link_url: Option<String>,
    image_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostDetail {
    id: i32,
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
    content_visible: bool,
    has_link: bool,
    has_image: bool,
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

#[derive(Clone, Debug, Serialize, FromRow)]
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
    content: Option<String>,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    sign_id: Option<i32>,
}

#[derive(Debug, FromRow)]
struct PostListDetailRow {
    id: i32,
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
    content: Option<String>,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    signature_id: Option<i32>,
    signature_content: Option<String>,
}

#[derive(Debug, FromRow)]
struct PostPointAwardRow {
    post_id: i32,
    user_id: i32,
    user_name: Option<String>,
    point: i32,
}

pub async fn post(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let can_read_encrypted = auth::is_authenticated(&state, &headers).await?;
    let row = post_detail(&state, post_id).await?;
    let tree = post_tree(&state, row.root_id, can_read_encrypted).await?;
    let boards = board_navigation(&state).await?;
    let (board, post) = hydrate_post(&state, row, can_read_encrypted).await?;

    Ok(no_store_json(PostResponse {
        site_name: state.site_name.clone(),
        board,
        tree,
        boards,
        post,
    }))
}

pub async fn post_print(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let can_read_encrypted = auth::is_authenticated(&state, &headers).await?;
    let row = post_detail(&state, post_id).await?;
    let (board, post) = hydrate_post(&state, row, can_read_encrypted).await?;

    Ok(no_store_json(PostPrintResponse {
        site_name: state.site_name.clone(),
        board,
        post,
    }))
}

pub async fn post_list(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let can_read_encrypted = auth::is_authenticated(&state, &headers).await?;
    let selected = post_detail(&state, post_id).await?;
    let rows = post_list_details(&state, selected.root_id).await?;
    let point_awards = post_list_point_awards(&state, &rows, can_read_encrypted).await?;
    let boards = board_navigation(&state).await?;

    let posts = rows
        .into_iter()
        .map(|row| {
            let content_visible = can_view_content(row.state, can_read_encrypted);
            PostDetail {
                id: row.id,
                level: row.level,
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
                content_visible,
                has_link: row.link_url.is_some(),
                has_image: row.image_url.is_some(),
                content: content_visible.then_some(row.content).flatten(),
                link_name: content_visible.then_some(row.link_name).flatten(),
                link_url: content_visible.then_some(row.link_url).flatten(),
                image_url: content_visible.then_some(row.image_url).flatten(),
                signature: content_visible
                    .then_some(row.signature_id)
                    .flatten()
                    .map(|id| SignatureSummary {
                        id,
                        content: row.signature_content,
                    }),
                point_awards: point_awards.get(&row.id).cloned().unwrap_or_default(),
            }
        })
        .collect();

    Ok(no_store_json(PostListResponse {
        site_name: state.site_name.clone(),
        selected_post_id: selected.id,
        board: PostBoard {
            id: selected.board_id,
            name: selected.board_name,
        },
        posts,
        boards,
    }))
}

fn no_store_json<T: Serialize>(body: T) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

async fn post_detail(state: &AppState, post_id: i32) -> AppResult<PostDetailRow> {
    let row = sqlx::query_as::<_, PostDetailRow>(
        r#"
        SELECT
            p.id,
            p.board_id,
            BTRIM(b.name) AS board_name,
            COALESCE(p.root_id, p.id) AS root_id,
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
            NULLIF(p.content, '') AS content,
            NULLIF(BTRIM(p.link_name), '') AS link_name,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url,
            p.sign_id
        FROM post p
        JOIN board b ON b.id = p.board_id
        WHERE p.id = $1
          AND p.state IN (0, 1)
        "#,
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await?;

    row.ok_or(AppError::NotFound)
}

async fn hydrate_post(
    state: &AppState,
    row: PostDetailRow,
    can_read_encrypted: bool,
) -> AppResult<(PostBoard, PostDetail)> {
    let content_visible = can_view_content(row.state, can_read_encrypted);
    let signature = match row.sign_id.filter(|_| content_visible) {
        Some(sign_id) => signature(state, sign_id).await?,
        None => None,
    };
    let point_awards = if content_visible && row.point.unwrap_or(0) != 0 {
        point_awards(state, row.id).await?
    } else {
        Vec::new()
    };

    Ok((
        PostBoard {
            id: row.board_id,
            name: row.board_name,
        },
        PostDetail {
            id: row.id,
            level: row.level,
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
            content_visible,
            has_link: row.link_url.is_some(),
            has_image: row.image_url.is_some(),
            content: content_visible.then_some(row.content).flatten(),
            link_name: content_visible.then_some(row.link_name).flatten(),
            link_url: content_visible.then_some(row.link_url).flatten(),
            image_url: content_visible.then_some(row.image_url).flatten(),
            signature,
            point_awards,
        },
    ))
}

fn can_view_content(post_state: i32, authenticated: bool) -> bool {
    post_state == 0 || (post_state == 1 && authenticated)
}

async fn post_list_details(state: &AppState, root_id: i32) -> AppResult<Vec<PostListDetailRow>> {
    let rows = sqlx::query_as::<_, PostListDetailRow>(
        r#"
        SELECT
            p.id,
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
            NULLIF(p.content, '') AS content,
            NULLIF(BTRIM(p.link_name), '') AS link_name,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url,
            signature.id AS signature_id,
            NULLIF(signature.content, '') AS signature_content
        FROM post p
        LEFT JOIN post signature ON signature.id = p.sign_id AND signature.state = 0
        WHERE COALESCE(p.root_id, p.id) = $1
          AND p.state IN (0, 1)
        ORDER BY p.order_num
        "#,
    )
    .bind(root_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows)
}

async fn post_list_point_awards(
    state: &AppState,
    posts: &[PostListDetailRow],
    can_read_encrypted: bool,
) -> AppResult<HashMap<i32, Vec<PointAward>>> {
    let post_ids = posts
        .iter()
        .filter(|post| {
            can_view_content(post.state, can_read_encrypted) && post.point.unwrap_or(0) != 0
        })
        .map(|post| post.id)
        .collect::<Vec<_>>();
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, PostPointAwardRow>(
        r#"
        SELECT
            pl.post_id,
            pl.user_id,
            NULLIF(BTRIM(u.name), '') AS user_name,
            pl.point
        FROM point_log pl
        LEFT JOIN user_info u ON u.id = pl.user_id
        WHERE pl.post_id = ANY($1)
        ORDER BY pl.post_id, pl.post_time, pl.id
        "#,
    )
    .bind(&post_ids[..])
    .fetch_all(&state.pool)
    .await?;

    let mut awards = HashMap::<i32, Vec<PointAward>>::new();
    for row in rows {
        awards.entry(row.post_id).or_default().push(PointAward {
            user_id: row.user_id,
            user_name: row.user_name,
            point: row.point,
        });
    }

    Ok(awards)
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

async fn post_tree(
    state: &AppState,
    root_id: i32,
    can_read_encrypted: bool,
) -> AppResult<PostTree> {
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
            NULLIF(BTRIM(p.link_url), '') IS NOT NULL AS has_link,
            NULLIF(BTRIM(p.image_url), '') IS NOT NULL AS has_image,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $2) THEN NULLIF(BTRIM(p.link_url), '') END AS link_url,
            CASE WHEN p.state = 0 OR (p.state = 1 AND $2) THEN NULLIF(BTRIM(p.image_url), '') END AS image_url
        FROM post p
        WHERE COALESCE(p.root_id, p.id) = $1
          AND p.state IN (0, 1)
        ORDER BY p.order_num
        "#,
    )
    .bind(root_id)
    .bind(can_read_encrypted)
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
