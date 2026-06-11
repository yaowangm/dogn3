use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::io::Cursor;

use image::{
    DynamicImage, ImageFormat, ImageReader, Limits, Rgb, RgbImage, codecs::jpeg::JpegEncoder,
    imageops,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};
use tokio::sync::Semaphore;

use crate::{
    auth::AuthenticatedUser,
    error::AppResult,
    routes::{auth, home, navigation},
    state::AppState,
};
use navigation::BoardNavSummary;

const ADMIN_LEVEL: i32 = 10;
const SIGNATURE_LOCK_NAMESPACE: i32 = 1_397_316_430;
const IMAGE_COMPRESSION_THRESHOLD_BYTES: usize = 500 * 1024;
const COMPRESSED_IMAGE_MAX_BYTES: usize = 500 * 1024;
const IMAGE_MAX_DIMENSION: u32 = 16_384;
const IMAGE_MAX_DECODED_BYTES: u64 = 128 * 1024 * 1024;
const IMAGE_MAX_CONCURRENT_PROCESSING: usize = 2;
static IMAGE_PROCESSING_PERMITS: Semaphore = Semaphore::const_new(IMAGE_MAX_CONCURRENT_PROCESSING);

#[derive(Debug, Deserialize)]
pub struct PostEditorQuery {
    board_id: Option<i32>,
    post_id: Option<i32>,
    reply_to: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SavePostRequest {
    board_id: Option<i32>,
    post_id: Option<i32>,
    parent_id: Option<i32>,
    subject: String,
    content: Option<String>,
    content_format: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    points: Option<i32>,
    image_content_type: Option<String>,
    image_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FavoritePostRequest {
    favorited: bool,
}

#[derive(Debug, Serialize)]
struct PostEditorResponse {
    site_name: String,
    mode: &'static str,
    board: EditorBoard,
    post: Option<EditorPost>,
    parent: Option<EditorPost>,
    boards: Vec<BoardNavSummary>,
    can_update_type: bool,
    post_subject_max_length: usize,
    post_content_max_bytes: usize,
    post_reply_max_points: i32,
    root_post_regular_award_points: i32,
    root_post_forward_award_points: i32,
    root_post_original_award_points: i32,
    current_user_points: i32,
    reply_points_allowed: bool,
    image_upload_max_bytes: usize,
}

#[derive(Debug, Serialize, FromRow)]
struct EditorBoard {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize, FromRow)]
struct EditorPost {
    id: i32,
    board_id: i32,
    parent_id: Option<i32>,
    root_id: i32,
    level: i32,
    subject: Option<String>,
    content: Option<String>,
    content_format: i32,
    post_type: Option<i32>,
    state: i32,
    image_url: Option<String>,
    user_id: Option<i32>,
}

#[derive(Debug, Serialize)]
struct SavedPostResponse {
    saved: bool,
    created: bool,
    post_id: i32,
}

#[derive(Debug, Serialize)]
struct DeletedPostResponse {
    deleted: bool,
    post_id: i32,
    board_id: i32,
    deleted_post_count: u64,
}

#[derive(Debug, Serialize)]
struct FavoritePostResponse {
    favorited: bool,
    post_id: i32,
    favorite_count: i32,
}

#[derive(Debug, Serialize)]
struct SignaturePostResponse {
    signature_set: bool,
    post_id: i32,
}

#[derive(Debug, Serialize)]
struct PostMutationErrorResponse {
    error: PostMutationError,
}

#[derive(Debug, Serialize)]
struct PostMutationError {
    code: &'static str,
    message: &'static str,
}

struct ValidatedPostInput {
    subject: String,
    content: Option<String>,
    size: i32,
    content_format: i32,
    post_type: i32,
    state: i32,
}

struct PendingImage {
    relative_path: String,
    destination: std::path::PathBuf,
    retained: bool,
}

impl PendingImage {
    fn retain(&mut self) {
        self.retained = true;
    }
}

impl Drop for PendingImage {
    fn drop(&mut self) {
        if !self.retained {
            let _ = std::fs::remove_file(&self.destination);
        }
    }
}

#[derive(Debug, FromRow)]
struct ReplyParent {
    id: i32,
    board_id: i32,
    parent_id: Option<i32>,
    root_id: i32,
    level: i32,
    order_num: i32,
    user_id: Option<i32>,
}

#[derive(Debug, FromRow)]
struct DeleteTarget {
    id: i32,
    board_id: i32,
    root_id: i32,
    user_id: Option<i32>,
}

enum PointTransferFailure {
    Database(sqlx::Error),
    Response(Response),
}

impl From<sqlx::Error> for PointTransferFailure {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub async fn editor(
    Query(query): Query<PostEditorQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(post_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to write a post.",
        ));
    };

    let (mode, board, post, parent, reply_points_allowed, can_update_type) = match (
        query.board_id,
        query.post_id,
        query.reply_to,
    ) {
        (Some(board_id), None, None) => (
            "create",
            editor_board(&state, board_id).await?,
            None,
            None,
            false,
            true,
        ),
        (None, Some(post_id), None) => {
            let post = editor_post(&state, post_id).await?;
            if !may_update_post(&viewer, &post)
                || signature_update_is_locked(&state, &viewer, post.id).await?
            {
                return Ok(post_error(
                    StatusCode::FORBIDDEN,
                    "not_authorized",
                    "You are not authorized to update this post.",
                ));
            }
            let board = editor_board(&state, post.board_id).await?;
            let can_update_type = viewer.level >= ADMIN_LEVEL
                && post_is_root(post.id, post.parent_id, post.root_id, post.level);
            ("update", board, Some(post), None, false, can_update_type)
        }
        (None, None, Some(parent_id)) => {
            let parent = editor_post(&state, parent_id).await?;
            if !reply_tree_is_open(&state, parent_id).await? {
                return Ok(reply_closed_error());
            }
            let board = editor_board(&state, parent.board_id).await?;
            let reply_points_allowed =
                post_is_root(parent.id, parent.parent_id, parent.root_id, parent.level)
                    && parent.user_id != Some(viewer.id);
            (
                "reply",
                board,
                None,
                Some(parent),
                reply_points_allowed,
                false,
            )
        }
        _ => {
            return Ok(post_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_target",
                "Select exactly one board for a new post, one post to update, or one post to reply to.",
            ));
        }
    };

    Ok(no_store_json(PostEditorResponse {
        site_name: state.site_name.clone(),
        mode,
        board,
        post,
        parent,
        boards: navigation::boards(&state).await?,
        can_update_type,
        post_subject_max_length: state.post_subject_max_length,
        post_content_max_bytes: state.post_content_max_bytes,
        post_reply_max_points: state.post_reply_max_points,
        root_post_regular_award_points: state.root_post_regular_award_points,
        root_post_forward_award_points: state.root_post_forward_award_points,
        root_post_original_award_points: state.root_post_original_award_points,
        current_user_points: current_user_points(&state, viewer.id).await?,
        reply_points_allowed,
        image_upload_max_bytes: state.image_upload_max_bytes,
    }))
}

pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SavePostRequest>,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(post_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to write a post.",
        ));
    };
    match (request.board_id, request.post_id, request.parent_id) {
        (Some(board_id), None, None) => {
            if request.points.unwrap_or(0) != 0 {
                return Ok(points_only_for_reply_error());
            }
            let input = match validate_input(&state, &request, request.post_type.unwrap_or(-1)) {
                Ok(input) => input,
                Err(response) => return Ok(response),
            };
            if !board_exists_in_pool(&state, board_id).await? {
                return Ok(post_error(
                    StatusCode::NOT_FOUND,
                    "board_not_found",
                    "The requested board was not found.",
                ));
            }
            let image = match prepare_request_image(&state, &request).await {
                Ok(image) => image,
                Err(response) => return Ok(response),
            };
            create_post(&state, &viewer, board_id, input, image).await
        }
        (None, Some(post_id), None) => {
            if request.points.unwrap_or(0) != 0 {
                return Ok(points_only_for_reply_error());
            }
            if request.image_hex.is_some() || request.image_content_type.is_some() {
                return Ok(post_error(
                    StatusCode::CONFLICT,
                    "image_update_not_allowed",
                    "An attached image cannot be added or replaced while updating a post.",
                ));
            }
            update_post(&state, &viewer, post_id, &request).await
        }
        (None, None, Some(parent_id)) => {
            if request.post_type.is_some_and(|post_type| post_type != 0) {
                return Ok(post_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_post_option",
                    "Replies are normal posts and cannot set a post type.",
                ));
            }
            let input = match validate_input(&state, &request, 0) {
                Ok(input) => input,
                Err(response) => return Ok(response),
            };
            let points = match validate_reply_points(&state, request.points.unwrap_or(0)) {
                Ok(points) => points,
                Err(response) => return Ok(response),
            };
            if !reply_target_is_open(&state, parent_id).await? {
                return Ok(reply_closed_error());
            }
            let image = match prepare_request_image(&state, &request).await {
                Ok(image) => image,
                Err(response) => return Ok(response),
            };
            reply_to_post(&state, &viewer, parent_id, input, points, image).await
        }
        _ => Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_target",
            "Select exactly one board for a new post, one post to update, or one post to reply to.",
        )),
    }
}

pub async fn delete(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(post_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to delete a post.",
        ));
    };

    let mut transaction = state.pool.begin().await?;
    let Some(target) = sqlx::query_as::<_, DeleteTarget>(
        r#"
        SELECT id, board_id, COALESCE(root_id, id) AS root_id, user_id
        FROM post
        WHERE id = $1 AND state IN (0, 1)
        FOR UPDATE
        "#,
    )
    .bind(post_id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::NOT_FOUND,
            "post_not_found",
            "The requested post was not found.",
        ));
    };

    let root_post = target.id == target.root_id;
    let tree_post_count = if root_post {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM post WHERE COALESCE(root_id, id) = $1")
            .bind(target.root_id)
            .fetch_one(&mut *transaction)
            .await?
    } else {
        1
    };
    if !may_delete_post(&mut transaction, &viewer, &target, tree_post_count).await? {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to delete this post.",
        ));
    }

    let deleted_rows = if root_post {
        sqlx::query_as::<_, (i32, Option<i32>)>(
            r#"
            UPDATE post
            SET state = 2
            WHERE COALESCE(root_id, id) = $1
              AND state <> 2
            RETURNING id, user_id
            "#,
        )
        .bind(target.root_id)
        .fetch_all(&mut *transaction)
        .await?
    } else {
        sqlx::query_as::<_, (i32, Option<i32>)>(
            "UPDATE post SET state = 2 WHERE id = $1 RETURNING id, user_id",
        )
        .bind(post_id)
        .fetch_all(&mut *transaction)
        .await?
    };
    let deleted_post_ids = deleted_rows.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let author_ids = deleted_rows
        .into_iter()
        .filter_map(|(_, user_id)| user_id)
        .collect::<Vec<_>>();
    refresh_deleted_post_statistics(
        &mut transaction,
        target.board_id,
        &deleted_post_ids,
        &author_ids,
    )
    .await?;
    transaction.commit().await?;
    home::invalidate_cache(&state).await;

    Ok(no_store_json(DeletedPostResponse {
        deleted: true,
        post_id,
        board_id: target.board_id,
        deleted_post_count: deleted_post_ids.len() as u64,
    }))
}

pub async fn favorite(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<FavoritePostRequest>,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(post_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to update favorites.",
        ));
    };

    let mut transaction = state.pool.begin().await?;
    let valid_target: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post
            WHERE id = $1
              AND id = COALESCE(root_id, id)
              AND state IN (0, 1)
        )
        "#,
    )
    .bind(post_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !valid_target {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_favorite_target",
            "Only a visible root post can be favorited.",
        ));
    }
    if request.favorited {
        sqlx::query(
            r#"
            INSERT INTO favorite (user_id, post_id, create_time)
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            ON CONFLICT (user_id, post_id) DO NOTHING
            "#,
        )
        .bind(viewer.id)
        .bind(post_id)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query("DELETE FROM favorite WHERE user_id = $1 AND post_id = $2")
            .bind(viewer.id)
            .bind(post_id)
            .execute(&mut *transaction)
            .await?;
    }
    let favorite_count: Option<i32> = sqlx::query_scalar(
        r#"
        UPDATE user_info AS u
        SET favorite_count = (
            SELECT COUNT(*)::integer
            FROM favorite f
            JOIN post p ON p.id = f.post_id
            WHERE f.user_id = u.id AND p.state IN (0, 1)
        )
        WHERE u.id = $1
        RETURNING favorite_count
        "#,
    )
    .bind(viewer.id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(no_store_json(FavoritePostResponse {
        favorited: request.favorited,
        post_id,
        favorite_count: favorite_count.unwrap_or(0),
    }))
}

pub async fn signature(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(post_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to set a signature.",
        ));
    };
    let Some(size) = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT size FROM post WHERE id = $1 AND state IN (0, 1)",
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await?
    else {
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_signature_target",
            "Only a visible post can be used as a signature.",
        ));
    };
    if size.unwrap_or(0).max(0) as usize > state.post_signature_max_bytes {
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "signature_too_large",
            "This post exceeds the configured signature size limit.",
        ));
    }
    let mut transaction = state.pool.begin().await?;
    lock_signature_target(&mut transaction, post_id).await?;
    let current_signature: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT sign_id
        FROM sign_log
        WHERE user_id = $1
        ORDER BY set_time DESC NULLS LAST, id DESC
        LIMIT 1
        "#,
    )
    .bind(viewer.id)
    .fetch_optional(&mut *transaction)
    .await?;
    if current_signature != Some(post_id) {
        sqlx::query(
            r#"
            INSERT INTO sign_log (user_id, sign_id, set_time)
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(viewer.id)
        .bind(post_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;

    Ok(no_store_json(SignaturePostResponse {
        signature_set: true,
        post_id,
    }))
}

async fn create_post(
    state: &AppState,
    viewer: &AuthenticatedUser,
    board_id: i32,
    input: ValidatedPostInput,
    mut image: Option<PendingImage>,
) -> AppResult<Response> {
    let mut transaction = state.pool.begin().await?;
    if !board_exists(&mut transaction, board_id).await? {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    }
    let root_award = root_post_award(&mut transaction, state, viewer.id, input.post_type).await?;
    let post_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO post (
            subject, board_id, user_id, user_name, post_time, reply_time,
            size, reply_count, access_count, point, type, state, content, content_format,
            link_name, link_url, image_url, parent_id, level, order_num
        )
        VALUES (
            $1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
            $5, 1, 0, 0, $6, $7, $8, $9, NULL, NULL, $10, 0, 0, 0
        )
        RETURNING id
        "#,
    )
    .bind(input.subject)
    .bind(board_id)
    .bind(viewer.id)
    .bind(&viewer.name)
    .bind(input.size)
    .bind(input.post_type)
    .bind(input.state)
    .bind(input.content)
    .bind(input.content_format)
    .bind(image.as_ref().map(|image| image.relative_path.as_str()))
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query("UPDATE post SET root_id = id WHERE id = $1")
        .bind(post_id)
        .execute(&mut *transaction)
        .await?;
    if let Some(points) = root_award {
        sqlx::query("UPDATE user_info SET point = COALESCE(point, 0) + $1 WHERE id = $2")
            .bind(points)
            .bind(viewer.id)
            .execute(&mut *transaction)
            .await?;
    }
    refresh_statistics(&mut transaction, board_id, viewer.id).await?;
    transaction.commit().await?;
    if let Some(image) = image.as_mut() {
        image.retain();
    }
    home::invalidate_cache(state).await;

    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(SavedPostResponse {
            saved: true,
            created: true,
            post_id,
        }),
    )
        .into_response())
}

async fn update_post(
    state: &AppState,
    viewer: &AuthenticatedUser,
    post_id: i32,
    request: &SavePostRequest,
) -> AppResult<Response> {
    let mut transaction = state.pool.begin().await?;
    let Some(existing) = sqlx::query_as::<_, EditorPost>(
        r#"
        SELECT
            id, board_id, parent_id, COALESCE(root_id, id) AS root_id,
            level, subject, content, COALESCE(content_format, 0) AS content_format, type AS post_type, state,
            image_url, user_id
        FROM post
        WHERE id = $1 AND state IN (0, 1)
        FOR UPDATE
        "#,
    )
    .bind(post_id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::NOT_FOUND,
            "post_not_found",
            "The requested post was not found.",
        ));
    };
    if !may_update_post(viewer, &existing) {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to update this post.",
        ));
    }
    if viewer.level < ADMIN_LEVEL {
        lock_signature_target(&mut transaction, post_id).await?;
        if signature_history_exists(&mut transaction, post_id).await? {
            transaction.rollback().await?;
            return Ok(post_error(
                StatusCode::FORBIDDEN,
                "signature_post_locked",
                "A post that has been used as a signature cannot be updated.",
            ));
        }
    }
    let update_post_type = if post_is_root(
        existing.id,
        existing.parent_id,
        existing.root_id,
        existing.level,
    ) {
        if viewer.level >= ADMIN_LEVEL {
            request.post_type.unwrap_or(-1)
        } else {
            existing.post_type.unwrap_or(0)
        }
    } else {
        0
    };
    let input = match validate_input_with_default_format(
        state,
        request,
        update_post_type,
        existing.content_format,
    ) {
        Ok(input) => input,
        Err(response) => {
            transaction.rollback().await?;
            return Ok(response);
        }
    };
    if input.content_format != existing.content_format {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "content_format_immutable",
            "Post content format cannot be changed after publication.",
        ));
    }

    sqlx::query(
        r#"
        UPDATE post
        SET subject = $1,
            content = $2,
            size = $3,
            content_format = $4,
            type = $5,
            state = $6,
            last_update_time = CURRENT_TIMESTAMP
        WHERE id = $7
        "#,
    )
    .bind(input.subject)
    .bind(input.content)
    .bind(input.size)
    .bind(input.content_format)
    .bind(input.post_type)
    .bind(input.state)
    .bind(post_id)
    .execute(&mut *transaction)
    .await?;
    if let Some(user_id) = existing.user_id {
        refresh_statistics(&mut transaction, existing.board_id, user_id).await?;
    } else {
        refresh_board_statistics(&mut transaction, existing.board_id).await?;
    }
    transaction.commit().await?;
    home::invalidate_cache(state).await;

    Ok(no_store_json(SavedPostResponse {
        saved: true,
        created: false,
        post_id,
    }))
}

async fn reply_to_post(
    state: &AppState,
    viewer: &AuthenticatedUser,
    parent_id: i32,
    input: ValidatedPostInput,
    points: i32,
    mut image: Option<PendingImage>,
) -> AppResult<Response> {
    let mut transaction = state.pool.begin().await?;
    let Some(root_id) = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT COALESCE(root_id, id)
        FROM post
        WHERE id = $1 AND state IN (0, 1)
        "#,
    )
    .bind(parent_id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.rollback().await?;
        return Ok(reply_closed_error());
    };
    let root_is_open: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT post_time >= CURRENT_TIMESTAMP - ($2 * INTERVAL '1 day')
        FROM post
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(root_id)
    .bind(state.post_reply_max_age_days)
    .fetch_optional(&mut *transaction)
    .await?;
    if !root_is_open.unwrap_or(false) {
        transaction.rollback().await?;
        return Ok(reply_closed_error());
    }
    let Some(parent) = sqlx::query_as::<_, ReplyParent>(
        r#"
        SELECT id, board_id, parent_id, COALESCE(root_id, id) AS root_id, level, order_num, user_id
        FROM post
        WHERE id = $1 AND COALESCE(root_id, id) = $2 AND state IN (0, 1)
        FOR UPDATE
        "#,
    )
    .bind(parent_id)
    .bind(root_id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.rollback().await?;
        return Ok(reply_closed_error());
    };
    if points > 0 {
        if !post_is_root(parent.id, parent.parent_id, parent.root_id, parent.level) {
            transaction.rollback().await?;
            return Ok(reply_points_only_for_root_error());
        }
        if let Err(error) = transfer_reply_points(&mut transaction, viewer, &parent, points).await {
            transaction.rollback().await?;
            return match error {
                PointTransferFailure::Database(error) => Err(error.into()),
                PointTransferFailure::Response(response) => Ok(response),
            };
        }
    }

    sqlx::query("UPDATE post SET order_num = order_num + 1 WHERE root_id = $1 AND order_num > $2")
        .bind(parent.root_id)
        .bind(parent.order_num)
        .execute(&mut *transaction)
        .await?;
    let post_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO post (
            subject, board_id, user_id, user_name, post_time, reply_time,
            size, reply_count, access_count, point, type, state, content, content_format,
            link_name, link_url, image_url, parent_id, root_id, level, order_num
        )
        VALUES (
            $1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
            $5, 0, 0, 0, 0, $6, $7, $8, NULL, NULL, $9, $10, $11, $12, $13
        )
        RETURNING id
        "#,
    )
    .bind(input.subject)
    .bind(parent.board_id)
    .bind(viewer.id)
    .bind(&viewer.name)
    .bind(input.size)
    .bind(input.state)
    .bind(input.content)
    .bind(input.content_format)
    .bind(image.as_ref().map(|image| image.relative_path.as_str()))
    .bind(parent.id)
    .bind(parent.root_id)
    .bind(parent.level + 1)
    .bind(parent.order_num + 1)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE post
        SET reply_count = (
                SELECT COUNT(*)::integer
                FROM post AS tree_post
                WHERE COALESCE(tree_post.root_id, tree_post.id) = $1
            ),
            reply_time = CURRENT_TIMESTAMP
        WHERE id = $1
        "#,
    )
    .bind(parent.root_id)
    .execute(&mut *transaction)
    .await?;
    refresh_statistics(&mut transaction, parent.board_id, viewer.id).await?;
    transaction.commit().await?;
    if let Some(image) = image.as_mut() {
        image.retain();
    }
    home::invalidate_cache(state).await;

    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(SavedPostResponse {
            saved: true,
            created: true,
            post_id,
        }),
    )
        .into_response())
}

async fn transfer_reply_points(
    transaction: &mut Transaction<'_, Postgres>,
    viewer: &AuthenticatedUser,
    parent: &ReplyParent,
    points: i32,
) -> Result<(), PointTransferFailure> {
    let Some(recipient_id) = parent.user_id else {
        return Err(PointTransferFailure::Response(post_error(
            StatusCode::CONFLICT,
            "point_recipient_unavailable",
            "The post owner cannot receive points.",
        )));
    };
    let self_transfer = recipient_id == viewer.id;
    if self_transfer {
        return Err(PointTransferFailure::Response(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "self_point_transfer",
            "Points cannot be transferred to your own post.",
        )));
    }
    let accounts: Vec<i32> =
        sqlx::query_scalar("SELECT id FROM user_info WHERE id IN ($1, $2) ORDER BY id FOR UPDATE")
            .bind(viewer.id)
            .bind(recipient_id)
            .fetch_all(&mut **transaction)
            .await?;
    if accounts.len() != 2 {
        return Err(PointTransferFailure::Response(post_error(
            StatusCode::CONFLICT,
            "point_recipient_unavailable",
            "The post owner cannot receive points.",
        )));
    }
    let debited = sqlx::query(
        "UPDATE user_info SET point = COALESCE(point, 0) - $1 WHERE id = $2 AND COALESCE(point, 0) >= $1",
    )
    .bind(points)
    .bind(viewer.id)
    .execute(&mut **transaction)
    .await?
    .rows_affected();
    if debited != 1 {
        return Err(PointTransferFailure::Response(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "insufficient_points",
            "You do not have enough points for this transfer.",
        )));
    }
    sqlx::query("UPDATE user_info SET point = COALESCE(point, 0) + $1 WHERE id = $2")
        .bind(points)
        .bind(recipient_id)
        .execute(&mut **transaction)
        .await?;
    sqlx::query("UPDATE post SET point = COALESCE(point, 0) + $1 WHERE id = $2")
        .bind(points)
        .bind(parent.id)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(
        "INSERT INTO point_log (post_id, user_id, point, post_time) VALUES ($1, $2, $3, CURRENT_TIMESTAMP)",
    )
    .bind(parent.id)
    .bind(viewer.id)
    .bind(points)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn root_post_award(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    user_id: i32,
    post_type: i32,
) -> Result<Option<i32>, sqlx::Error> {
    let (award_category, points) = root_post_award_rule(state, post_type);
    sqlx::query("SELECT id FROM user_info WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
    let awarded_today: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post p
            WHERE p.user_id = $1
              AND COALESCE(p.parent_id, 0) = 0
              AND p.post_time >= CURRENT_DATE
              AND p.post_time < CURRENT_DATE + INTERVAL '1 day'
              AND (
                    ($2 = 1 AND p.type = 1)
                 OR ($2 = 2 AND p.type = 2)
                 OR ($2 = 0 AND COALESCE(p.type, 0) NOT IN (1, 2))
              )
        )
        "#,
    )
    .bind(user_id)
    .bind(award_category)
    .fetch_one(&mut **transaction)
    .await?;

    Ok((!awarded_today).then_some(points))
}

fn root_post_award_rule(state: &AppState, post_type: i32) -> (i32, i32) {
    match post_type {
        1 => (1, state.root_post_original_award_points),
        2 => (2, state.root_post_forward_award_points),
        _ => (0, state.root_post_regular_award_points),
    }
}

fn validate_input(
    state: &AppState,
    request: &SavePostRequest,
    post_type: i32,
) -> Result<ValidatedPostInput, Response> {
    validate_input_with_default_format(state, request, post_type, 0)
}

fn validate_input_with_default_format(
    state: &AppState,
    request: &SavePostRequest,
    post_type: i32,
    default_content_format: i32,
) -> Result<ValidatedPostInput, Response> {
    let subject = request.subject.trim().to_string();
    if subject.is_empty() || subject.chars().count() > state.post_subject_max_length {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_subject",
            "Post subject exceeds the configured length limit.",
        ));
    }
    let content_format = request.content_format.unwrap_or(default_content_format);
    if !matches!(post_type, 0..=3)
        || !matches!(request.state, 0..=1)
        || !matches!(content_format, 0..=1)
    {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_post_option",
            "Select a valid post type, visibility, and content format.",
        ));
    }
    let content = optional_content(request.content.clone());
    if content
        .as_ref()
        .is_some_and(|content| content.len() > state.post_content_max_bytes)
    {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "content_too_large",
            "Post content exceeds the configured size limit.",
        ));
    }
    let size = content
        .as_ref()
        .map(|content| i32::try_from(content.len()))
        .transpose()
        .map_err(|_| {
            post_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "content_too_large",
                "Post content is too large.",
            )
        })?
        .unwrap_or(0);
    Ok(ValidatedPostInput {
        subject,
        content,
        size,
        content_format,
        post_type,
        state: request.state,
    })
}

fn validate_reply_points(state: &AppState, points: i32) -> Result<i32, Response> {
    if !(0..=state.post_reply_max_points).contains(&points) {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_reply_points",
            "Reply points must be within the configured transfer limit.",
        ));
    }
    Ok(points)
}

fn optional_content(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

async fn editor_post(state: &AppState, post_id: i32) -> AppResult<EditorPost> {
    sqlx::query_as::<_, EditorPost>(
        r#"
        SELECT
            id, board_id, parent_id, COALESCE(root_id, id) AS root_id,
            level, subject, content, COALESCE(content_format, 0) AS content_format, type AS post_type, state,
            image_url, user_id
        FROM post
        WHERE id = $1 AND state IN (0, 1)
        "#,
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(crate::error::AppError::NotFound)
}

fn post_is_root(id: i32, parent_id: Option<i32>, root_id: i32, level: i32) -> bool {
    matches!(parent_id, None | Some(0)) || root_id == id || level == 0
}

async fn reply_tree_is_open(state: &AppState, post_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post AS root_post
            JOIN post AS selected_post
              ON selected_post.id = $1
             AND COALESCE(selected_post.root_id, selected_post.id) = root_post.id
            WHERE root_post.state IN (0, 1)
              AND selected_post.state IN (0, 1)
              AND root_post.post_time >= CURRENT_TIMESTAMP - ($2 * INTERVAL '1 day')
        )
        "#,
    )
    .bind(post_id)
    .bind(state.post_reply_max_age_days)
    .fetch_one(&state.pool)
    .await
}

async fn editor_board(state: &AppState, board_id: i32) -> AppResult<EditorBoard> {
    sqlx::query_as::<_, EditorBoard>("SELECT id, BTRIM(name) AS name FROM board WHERE id = $1")
        .bind(board_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(crate::error::AppError::NotFound)
}

async fn current_user_points(state: &AppState, user_id: i32) -> AppResult<i32> {
    let points = sqlx::query_scalar("SELECT COALESCE(point, 0) FROM user_info WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;

    Ok(points)
}

async fn current_upload_month(state: &AppState) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT to_char(CURRENT_DATE, 'YYYYMM')")
        .fetch_one(&state.pool)
        .await
}

fn random_image_file_name(extension: &str) -> String {
    use argon2::password_hash::rand_core::RngCore;

    let mut bytes = [0_u8; 16];
    argon2::password_hash::rand_core::OsRng.fill_bytes(&mut bytes);
    format!("{}.{}", hex(&bytes), extension)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

async fn board_exists(
    transaction: &mut Transaction<'_, Postgres>,
    board_id: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM board WHERE id = $1)")
        .bind(board_id)
        .fetch_one(&mut **transaction)
        .await
}

async fn refresh_statistics(
    transaction: &mut Transaction<'_, Postgres>,
    board_id: i32,
    user_id: i32,
) -> Result<(), sqlx::Error> {
    refresh_board_statistics(transaction, board_id).await?;
    sqlx::query(
        r#"
        WITH statistics AS (
            SELECT
                COUNT(*) FILTER (WHERE p.state IN (0, 1))::integer AS post_count,
                COUNT(*) FILTER (
                    WHERE p.type = 1 AND p.state IN (0, 1)
                )::integer AS doc_count,
                MAX(p.post_time) FILTER (WHERE p.state IN (0, 1)) AS last_post,
                MAX(p.post_time) FILTER (
                    WHERE p.type = 1 AND p.state IN (0, 1)
                ) AS last_origin,
                MAX(p.post_time) FILTER (
                    WHERE p.type = 2 AND p.state IN (0, 1)
                ) AS last_reship
            FROM post p
            WHERE p.user_id = $1
        )
        UPDATE user_info AS u
        SET post_count = statistics.post_count,
            doc_count = statistics.doc_count,
            last_post = statistics.last_post,
            last_origin = statistics.last_origin,
            last_reship = statistics.last_reship
        FROM statistics
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn refresh_board_statistics(
    transaction: &mut Transaction<'_, Postgres>,
    board_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        WITH statistics AS (
            SELECT
                COUNT(*) FILTER (WHERE p.state IN (0, 1))::integer AS post_count,
                COUNT(*) FILTER (
                    WHERE p.state IN (0, 1)
                      AND COALESCE(p.parent_id, 0) = 0
                )::integer AS root_count
            FROM post p
            WHERE p.board_id = $1
        )
        UPDATE board AS b
        SET post_count = statistics.post_count,
            root_count = statistics.root_count
        FROM statistics
        WHERE b.id = $1
        "#,
    )
    .bind(board_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn refresh_deleted_post_statistics(
    transaction: &mut Transaction<'_, Postgres>,
    board_id: i32,
    deleted_post_ids: &[i32],
    author_ids: &[i32],
) -> Result<(), sqlx::Error> {
    refresh_board_statistics(transaction, board_id).await?;
    sqlx::query(
        r#"
        WITH affected_users AS (
            SELECT UNNEST($1::integer[]) AS user_id
            UNION
            SELECT f.user_id
            FROM favorite f
            WHERE f.post_id = ANY($2)
        ),
        post_statistics AS (
            SELECT
                affected.user_id,
                COUNT(p.id) FILTER (WHERE p.state IN (0, 1))::integer AS post_count,
                COUNT(p.id) FILTER (
                    WHERE p.type = 1 AND p.state IN (0, 1)
                )::integer AS doc_count,
                MAX(p.post_time) FILTER (WHERE p.state IN (0, 1)) AS last_post,
                MAX(p.post_time) FILTER (
                    WHERE p.type = 1 AND p.state IN (0, 1)
                ) AS last_origin,
                MAX(p.post_time) FILTER (
                    WHERE p.type = 2 AND p.state IN (0, 1)
                ) AS last_reship
            FROM affected_users affected
            LEFT JOIN post p ON p.user_id = affected.user_id
            GROUP BY affected.user_id
        ),
        favorite_statistics AS (
            SELECT
                affected.user_id,
                COUNT(p.id)::integer AS favorite_count
            FROM affected_users affected
            LEFT JOIN favorite f ON f.user_id = affected.user_id
            LEFT JOIN post p ON p.id = f.post_id AND p.state IN (0, 1)
            GROUP BY affected.user_id
        )
        UPDATE user_info AS u
        SET post_count = post_statistics.post_count,
            doc_count = post_statistics.doc_count,
            last_post = post_statistics.last_post,
            last_origin = post_statistics.last_origin,
            last_reship = post_statistics.last_reship,
            favorite_count = favorite_statistics.favorite_count
        FROM post_statistics
        JOIN favorite_statistics USING (user_id)
        WHERE u.id = post_statistics.user_id
        "#,
    )
    .bind(author_ids)
    .bind(deleted_post_ids)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn board_exists_in_pool(state: &AppState, board_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM board WHERE id = $1)")
        .bind(board_id)
        .fetch_one(&state.pool)
        .await
}

async fn reply_target_is_open(state: &AppState, parent_id: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post AS parent
            JOIN post AS root
              ON root.id = COALESCE(parent.root_id, parent.id)
            WHERE parent.id = $1
              AND parent.state IN (0, 1)
              AND root.post_time >= CURRENT_TIMESTAMP - ($2 * INTERVAL '1 day')
        )
        "#,
    )
    .bind(parent_id)
    .bind(state.post_reply_max_age_days)
    .fetch_one(&state.pool)
    .await
}

async fn prepare_request_image(
    state: &AppState,
    request: &SavePostRequest,
) -> Result<Option<PendingImage>, Response> {
    let (Some(content_type), Some(encoded)) = (
        request.image_content_type.as_deref(),
        request.image_hex.as_deref(),
    ) else {
        if request.image_content_type.is_some() || request.image_hex.is_some() {
            return Err(post_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_image_type",
                "The image content type and image data must be provided together.",
            ));
        }
        return Ok(None);
    };
    if encoded.is_empty()
        || encoded.len() > state.image_upload_max_bytes.saturating_mul(2)
        || encoded.len() % 2 != 0
    {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_size",
            "The selected image exceeds the configured upload size limit.",
        ));
    }
    let body = decode_hex(encoded).ok_or_else(|| {
        post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_type",
            "The selected image data is invalid.",
        )
    })?;
    if body.is_empty() || body.len() > state.image_upload_max_bytes {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_size",
            "The selected image exceeds the configured upload size limit.",
        ));
    }
    let Some((format, original_extension)) = upload_format(content_type, &body) else {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_type",
            "Only JPG, PNG, and GIF images may be uploaded.",
        ));
    };
    let (stored_body, extension) = if body.len() > IMAGE_COMPRESSION_THRESHOLD_BYTES {
        let permit = IMAGE_PROCESSING_PERMITS.acquire().await.map_err(|error| {
            tracing::error!(?error, "image processing limiter closed");
            image_storage_error()
        })?;
        let compressed_body = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            compress_image(&body, format)
        })
        .await
        .map_err(|error| {
            tracing::error!(?error, "image compression task failed");
            image_storage_error()
        })?
        .map_err(|_| {
            post_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_image_type",
                "Only valid JPG, PNG, and GIF images may be uploaded.",
            )
        })?;
        (compressed_body, "jpg")
    } else {
        (body, original_extension)
    };
    let upload_month = current_upload_month(state).await.map_err(|error| {
        tracing::error!(?error, "failed to determine image upload month");
        image_storage_error()
    })?;
    let file_name = random_image_file_name(extension);
    let relative_path = format!("{upload_month}/{file_name}");
    let upload_directory = state.image_directory.join(&upload_month);
    tokio::fs::create_dir_all(&upload_directory)
        .await
        .map_err(|error| {
            tracing::error!(
                ?error,
                ?upload_directory,
                "failed to create image upload directory"
            );
            image_storage_error()
        })?;
    let destination = upload_directory.join(&file_name);
    tokio::fs::write(&destination, stored_body)
        .await
        .map_err(|error| {
            tracing::error!(?error, ?destination, "failed to write uploaded image");
            image_storage_error()
        })?;
    Ok(Some(PendingImage {
        relative_path,
        destination,
        retained: false,
    }))
}

fn decode_hex(encoded: &str) -> Option<Vec<u8>> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        decoded.push((hex_value(pair[0])? << 4) | hex_value(pair[1])?);
    }
    Some(decoded)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn upload_format(content_type: &str, body: &[u8]) -> Option<(ImageFormat, &'static str)> {
    match content_type {
        "image/jpeg" if body.starts_with(&[0xff, 0xd8, 0xff]) => Some((ImageFormat::Jpeg, "jpg")),
        "image/png" if body.starts_with(b"\x89PNG\r\n\x1a\n") => Some((ImageFormat::Png, "png")),
        "image/gif" if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") => {
            Some((ImageFormat::Gif, "gif"))
        }
        _ => None,
    }
}

fn compress_image(bytes: &[u8], format: ImageFormat) -> image::ImageResult<Vec<u8>> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(IMAGE_MAX_DIMENSION);
    limits.max_image_height = Some(IMAGE_MAX_DIMENSION);
    limits.max_alloc = Some(IMAGE_MAX_DECODED_BYTES);
    reader.limits(limits);
    let decoded = reader.decode()?;
    let mut image = flatten_transparency(decoded);
    let mut quality = 84_u8;

    loop {
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, quality)
            .encode_image(&DynamicImage::ImageRgb8(image.clone()))?;
        if encoded.len() < COMPRESSED_IMAGE_MAX_BYTES {
            return Ok(encoded);
        }

        if quality > 44 {
            quality -= 10;
            continue;
        }

        let width = image.width();
        let height = image.height();
        let reduced_width = ((width as f32 * 0.82).floor() as u32).max(1);
        let reduced_height = ((height as f32 * 0.82).floor() as u32).max(1);
        image = imageops::resize(
            &image,
            reduced_width,
            reduced_height,
            imageops::FilterType::Triangle,
        );
        quality = 74;
    }
}

fn flatten_transparency(image: DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = u16::from(pixel[3]);
        let blend =
            |channel: u8| ((u16::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        rgb.put_pixel(
            x,
            y,
            Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    rgb
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    use super::{
        COMPRESSED_IMAGE_MAX_BYTES, IMAGE_MAX_DIMENSION, compress_image, post_is_root,
        random_image_file_name,
    };

    #[test]
    fn compresses_large_upload_below_storage_limit() {
        let image = RgbImage::from_fn(1000, 1000, |x, y| {
            let seed = x
                .wrapping_mul(1_664_525)
                .wrapping_add(y.wrapping_mul(1_013_904_223));
            Rgb([
                seed as u8,
                (seed >> 8) as u8,
                (seed.rotate_left(11) >> 16) as u8,
            ])
        });
        let mut source = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut source), ImageFormat::Png)
            .expect("PNG fixture should encode");
        assert!(source.len() > COMPRESSED_IMAGE_MAX_BYTES);

        let compressed =
            compress_image(&source, ImageFormat::Png).expect("fixture should compress");

        assert!(compressed.len() < COMPRESSED_IMAGE_MAX_BYTES);
        assert!(compressed.starts_with(&[0xff, 0xd8, 0xff]));
    }

    #[test]
    fn rejects_image_dimensions_above_decoder_limit() {
        let image = RgbImage::new(IMAGE_MAX_DIMENSION + 1, 1);
        let mut source = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut source), ImageFormat::Png)
            .expect("PNG fixture should encode");

        assert!(compress_image(&source, ImageFormat::Png).is_err());
    }

    #[test]
    fn detects_root_posts_from_structural_fields() {
        assert!(post_is_root(10, Some(0), 10, 1));
        assert!(post_is_root(10, None, 10, 1));
        assert!(post_is_root(10, Some(99), 10, 2));
        assert!(post_is_root(10, Some(99), 99, 0));
        assert!(!post_is_root(10, Some(99), 99, 1));
    }

    #[test]
    fn random_image_file_names_are_unpredictable_hex_values() {
        let first = random_image_file_name("jpg");
        let second = random_image_file_name("jpg");

        assert_ne!(first, second);
        assert_eq!(first.len(), 36);
        assert!(first.ends_with(".jpg"));
        assert!(
            first[..32]
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
        );
    }
}

fn may_update_post(viewer: &AuthenticatedUser, post: &EditorPost) -> bool {
    viewer.level >= ADMIN_LEVEL
        || (post_is_root(post.id, post.parent_id, post.root_id, post.level)
            && post.user_id == Some(viewer.id))
}

async fn signature_update_is_locked(
    state: &AppState,
    viewer: &AuthenticatedUser,
    post_id: i32,
) -> Result<bool, sqlx::Error> {
    if viewer.level >= ADMIN_LEVEL {
        return Ok(false);
    }

    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sign_log WHERE sign_id = $1)")
        .bind(post_id)
        .fetch_one(&state.pool)
        .await
}

async fn signature_history_exists(
    transaction: &mut Transaction<'_, Postgres>,
    post_id: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sign_log WHERE sign_id = $1)")
        .bind(post_id)
        .fetch_one(&mut **transaction)
        .await
}

async fn lock_signature_target(
    transaction: &mut Transaction<'_, Postgres>,
    post_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
        .bind(SIGNATURE_LOCK_NAMESPACE)
        .bind(post_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn may_delete_post(
    transaction: &mut Transaction<'_, Postgres>,
    viewer: &AuthenticatedUser,
    target: &DeleteTarget,
    tree_post_count: i64,
) -> Result<bool, sqlx::Error> {
    if viewer.level >= ADMIN_LEVEL {
        return Ok(true);
    }

    let board_master: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM board_master WHERE board_id = $1 AND user_id = $2)",
    )
    .bind(target.board_id)
    .bind(viewer.id)
    .fetch_one(&mut **transaction)
    .await?;
    let owns_leaf_root =
        target.id == target.root_id && tree_post_count == 1 && target.user_id == Some(viewer.id);

    Ok(board_master || owns_leaf_root)
}

fn no_store_json<T: Serialize>(body: T) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

fn post_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(PostMutationErrorResponse {
            error: PostMutationError { code, message },
        }),
    )
        .into_response()
}

fn reply_closed_error() -> Response {
    post_error(
        StatusCode::CONFLICT,
        "reply_closed",
        "This post is no longer open for replies.",
    )
}

fn points_only_for_reply_error() -> Response {
    post_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_post_option",
        "Points can be transferred only when replying to a post.",
    )
}

fn image_storage_error() -> Response {
    post_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "image_storage_unavailable",
        "Image storage is not writable. Contact the site administrator.",
    )
}

fn reply_points_only_for_root_error() -> Response {
    post_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "reply_points_require_root",
        "Points can be transferred only when replying to a root post.",
    )
}
