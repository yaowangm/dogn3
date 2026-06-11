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
    auth::AuthenticatedUser,
    error::{AppError, AppResult},
    routes::{auth, navigation},
    state::AppState,
};
use navigation::BoardNavSummary;

const ADMIN_LEVEL: i32 = 10;

#[derive(Debug, Serialize)]
pub struct PostResponse {
    site_name: String,
    post: PostDetail,
    board: PostBoard,
    tree: PostTree,
    boards: Vec<BoardNavSummary>,
    can_update: bool,
    can_delete: bool,
    delete_post_count: i64,
    can_favorite: bool,
    is_favorite: bool,
    can_set_signature: bool,
    is_signature: bool,
    post_signature_max_bytes: usize,
    reply_open: bool,
    can_reply: bool,
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
    last_update_time: Option<String>,
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
    order_num: i32,
    subject: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_time: Option<String>,
    last_update_time: Option<String>,
    size: Option<i32>,
    reply_count: Option<i32>,
    access_count: i32,
    point: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    content_visible: bool,
    has_content: bool,
    has_link: bool,
    has_image: bool,
    content_format: i32,
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
    content_format: i32,
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
    order_num: i32,
    subject: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_time: Option<String>,
    last_update_time: Option<String>,
    size: Option<i32>,
    reply_count: Option<i32>,
    access_count: i32,
    point: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    content: Option<String>,
    content_format: i32,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    sign_id: Option<i32>,
}

#[derive(Debug, FromRow)]
struct PostListDetailRow {
    id: i32,
    level: i32,
    order_num: i32,
    subject: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_time: Option<String>,
    last_update_time: Option<String>,
    size: Option<i32>,
    reply_count: Option<i32>,
    access_count: i32,
    point: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    content: Option<String>,
    content_format: i32,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
    signature_id: Option<i32>,
    signature_content_format: i32,
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
    let (session, mut row) = tokio::try_join!(
        auth::current_session(&state, &headers),
        post_detail(&state, post_id),
    )?;
    let viewer = session.as_ref().map(|(_, user)| user.clone());
    let can_read_encrypted = viewer.is_some();
    if let Some((token, _)) = session.as_ref()
        && state.sessions.mark_post_viewed(token, row.id)
    {
        increment_access_count(&state, row.id).await?;
        row.access_count += 1;
    }
    let root_post = row.id == row.root_id;
    let (
        can_update,
        (can_delete, delete_post_count),
        (can_favorite, is_favorite),
        (can_set_signature, is_signature),
        reply_open,
        tree,
        boards,
    ) = tokio::try_join!(
        async {
            Ok::<_, AppError>(match viewer.as_ref() {
                Some(viewer) => update_capability(&state, viewer, &row).await?,
                None => false,
            })
        },
        async {
            Ok::<_, AppError>(match viewer.as_ref() {
                Some(viewer) => delete_capability(&state, viewer, &row).await?,
                None => (false, 0),
            })
        },
        async {
            Ok::<_, AppError>(match viewer.as_ref() {
                Some(viewer) if root_post => (true, has_favorite(&state, viewer.id, row.id).await?),
                _ => (false, false),
            })
        },
        async {
            Ok::<_, AppError>(match viewer.as_ref() {
                Some(viewer) if signature_size_is_allowed(&state, row.size) => {
                    (true, has_signature(&state, viewer.id, row.id).await?)
                }
                Some(_) | None => (false, false),
            })
        },
        async { Ok::<_, AppError>(reply_tree_is_open(&state, row.root_id).await?) },
        post_tree(&state, row.root_id, can_read_encrypted),
        navigation::boards(&state),
    )?;
    let can_reply = viewer.is_some() && reply_open;
    let (board, post) = hydrate_post(&state, row, can_read_encrypted).await?;

    Ok(no_store_json(PostResponse {
        site_name: state.site_name.clone(),
        board,
        tree,
        boards,
        post,
        can_update,
        can_delete,
        delete_post_count,
        can_favorite,
        is_favorite,
        can_set_signature,
        is_signature,
        post_signature_max_bytes: state.post_signature_max_bytes,
        reply_open,
        can_reply,
    }))
}

async fn increment_access_count(state: &AppState, post_id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE post SET access_count = access_count + 1 WHERE id = $1")
        .bind(post_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

async fn has_favorite(state: &AppState, user_id: i32, post_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM favorite WHERE user_id = $1 AND post_id = $2)")
        .bind(user_id)
        .bind(post_id)
        .fetch_one(&state.pool)
        .await
}

async fn update_capability(
    state: &AppState,
    viewer: &AuthenticatedUser,
    post: &PostDetailRow,
) -> Result<bool, sqlx::Error> {
    if viewer.level >= ADMIN_LEVEL {
        return Ok(true);
    }
    if post.level != 0 || post.user_id != Some(viewer.id) {
        return Ok(false);
    }
    let used_as_signature: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sign_log WHERE sign_id = $1)")
            .bind(post.id)
            .fetch_one(&state.pool)
            .await?;

    Ok(!used_as_signature)
}

async fn has_signature(state: &AppState, user_id: i32, post_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT sign_id = $2
        FROM sign_log
        WHERE user_id = $1
        ORDER BY set_time DESC NULLS LAST, id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await
    .map(|value| value.unwrap_or(false))
}

fn signature_size_is_allowed(state: &AppState, size: Option<i32>) -> bool {
    size.unwrap_or(0).max(0) as usize <= state.post_signature_max_bytes
}

async fn delete_capability(
    state: &AppState,
    viewer: &AuthenticatedUser,
    post: &PostDetailRow,
) -> Result<(bool, i64), sqlx::Error> {
    let root_post = post.id == post.root_id;
    let delete_post_count = if root_post {
        sqlx::query_scalar("SELECT COUNT(*) FROM post WHERE COALESCE(root_id, id) = $1")
            .bind(post.root_id)
            .fetch_one(&state.pool)
            .await?
    } else {
        1
    };
    let board_master: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM board_master WHERE board_id = $1 AND user_id = $2)",
    )
    .bind(post.board_id)
    .bind(viewer.id)
    .fetch_one(&state.pool)
    .await?;
    let owns_leaf_root = root_post && delete_post_count == 1 && post.user_id == Some(viewer.id);

    Ok((
        viewer.level >= ADMIN_LEVEL || board_master || owns_leaf_root,
        delete_post_count,
    ))
}

async fn reply_tree_is_open(state: &AppState, root_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post
            WHERE id = $1
              AND state IN (0, 1)
              AND post_time >= CURRENT_TIMESTAMP - ($2 * INTERVAL '1 day')
        )
        "#,
    )
    .bind(root_id)
    .bind(state.post_reply_max_age_days)
    .fetch_one(&state.pool)
    .await
}

pub async fn post_print(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let (can_read_encrypted, row) = tokio::try_join!(
        auth::is_authenticated(&state, &headers),
        post_detail(&state, post_id),
    )?;
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
    let (can_read_encrypted, selected) = tokio::try_join!(
        auth::is_authenticated(&state, &headers),
        post_detail(&state, post_id),
    )?;
    let (rows, boards) = tokio::try_join!(
        post_list_details(&state, selected.root_id, can_read_encrypted),
        navigation::boards(&state),
    )?;
    let point_awards = post_list_point_awards(&state, &rows, can_read_encrypted).await?;

    let posts = rows
        .into_iter()
        .map(|row| {
            let content_visible = can_view_content(row.state, can_read_encrypted);
            PostDetail {
                id: row.id,
                level: row.level,
                order_num: row.order_num,
                subject: row.subject,
                user_id: row.user_id,
                user_name: row.user_name,
                post_time: row.post_time,
                reply_time: row.reply_time,
                last_update_time: row.last_update_time,
                size: row.size,
                reply_count: row.reply_count,
                access_count: row.access_count,
                point: row.point,
                post_type: row.post_type,
                state: row.state,
                content_visible,
                has_content: content_visible && row.content.is_some(),
                has_link: row.link_url.is_some(),
                has_image: row.image_url.is_some(),
                content_format: row.content_format,
                content: content_visible.then_some(row.content).flatten(),
                link_name: content_visible.then_some(row.link_name).flatten(),
                link_url: content_visible.then_some(row.link_url).flatten(),
                image_url: content_visible.then_some(row.image_url).flatten(),
                signature: content_visible
                    .then_some(row.signature_id)
                    .flatten()
                    .map(|id| SignatureSummary {
                        id,
                        content_format: row.signature_content_format,
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
            p.order_num,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            to_char(p.reply_time, 'YYYY-MM-DD HH24:MI') AS reply_time,
            to_char(p.last_update_time, 'YYYY-MM-DD HH24:MI') AS last_update_time,
            p.size,
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(p.content, '') AS content,
            COALESCE(p.content_format, 0) AS content_format,
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
        Some(sign_id) => signature(state, sign_id, can_read_encrypted).await?,
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
            order_num: row.order_num,
            subject: row.subject,
            user_id: row.user_id,
            user_name: row.user_name,
            post_time: row.post_time,
            reply_time: row.reply_time,
            last_update_time: row.last_update_time,
            size: row.size,
            reply_count: row.reply_count,
            access_count: row.access_count,
            point: row.point,
            post_type: row.post_type,
            state: row.state,
            content_visible,
            has_content: content_visible && row.content.is_some(),
            has_link: row.link_url.is_some(),
            has_image: row.image_url.is_some(),
            content_format: row.content_format,
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

async fn post_list_details(
    state: &AppState,
    root_id: i32,
    can_read_encrypted: bool,
) -> AppResult<Vec<PostListDetailRow>> {
    let rows = sqlx::query_as::<_, PostListDetailRow>(
        r#"
        SELECT
            p.id,
            p.level,
            p.order_num,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.user_id,
            NULLIF(BTRIM(p.user_name), '') AS user_name,
            to_char(p.post_time, 'YYYY-MM-DD HH24:MI') AS post_time,
            to_char(p.reply_time, 'YYYY-MM-DD HH24:MI') AS reply_time,
            to_char(p.last_update_time, 'YYYY-MM-DD HH24:MI') AS last_update_time,
            p.size,
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(p.content, '') AS content,
            COALESCE(p.content_format, 0) AS content_format,
            NULLIF(BTRIM(p.link_name), '') AS link_name,
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url,
            signature.id AS signature_id,
            COALESCE(signature.content_format, 0) AS signature_content_format,
            NULLIF(signature.content, '') AS signature_content
        FROM post p
        LEFT JOIN post signature
          ON signature.id = p.sign_id
         AND (signature.state = 0 OR ($2 AND signature.state = 1))
        WHERE COALESCE(p.root_id, p.id) = $1
          AND p.state IN (0, 1)
        ORDER BY p.post_time ASC NULLS LAST, p.id ASC
        "#,
    )
    .bind(root_id)
    .bind(can_read_encrypted)
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

async fn signature(
    state: &AppState,
    sign_id: i32,
    can_read_encrypted: bool,
) -> AppResult<Option<SignatureSummary>> {
    let signature = sqlx::query_as::<_, SignatureSummary>(
        r#"
        SELECT id, COALESCE(content_format, 0) AS content_format, NULLIF(content, '') AS content
        FROM post
        WHERE id = $1
          AND (state = 0 OR ($2 AND state = 1))
        "#,
    )
    .bind(sign_id)
    .bind(can_read_encrypted)
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
            to_char(p.last_update_time, 'YYYY-MM-DD HH24:MI') AS last_update_time,
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
