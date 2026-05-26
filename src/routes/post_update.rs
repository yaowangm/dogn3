use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, codecs::jpeg::JpegEncoder, imageops};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};

use crate::{
    auth::AuthenticatedUser,
    error::AppResult,
    routes::{auth, home},
    state::AppState,
};

const ADMIN_LEVEL: i32 = 10;
const IMAGE_COMPRESSION_THRESHOLD_BYTES: usize = 500 * 1024;
const COMPRESSED_IMAGE_MAX_BYTES: usize = 500 * 1024;

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
    post_type: Option<i32>,
    state: i32,
}

#[derive(Debug, Serialize)]
struct PostEditorResponse {
    site_name: String,
    mode: &'static str,
    board: EditorBoard,
    post: Option<EditorPost>,
    parent: Option<EditorPost>,
    boards: Vec<BoardNavSummary>,
    post_subject_max_length: usize,
    post_content_max_bytes: usize,
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
    level: i32,
    subject: Option<String>,
    content: Option<String>,
    post_type: Option<i32>,
    state: i32,
    image_url: Option<String>,
    user_id: Option<i32>,
}

#[derive(Debug, Serialize, FromRow)]
struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Serialize)]
struct SavedPostResponse {
    saved: bool,
    created: bool,
    post_id: i32,
}

#[derive(Debug, Serialize)]
struct UploadedImageResponse {
    uploaded: bool,
    post_id: i32,
    image_url: String,
    compressed: bool,
    stored_bytes: usize,
}

#[derive(Debug, Serialize)]
struct DeletedPostResponse {
    deleted: bool,
    post_id: i32,
    board_id: i32,
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
    post_type: i32,
    state: i32,
}

#[derive(Debug, FromRow)]
struct ReplyParent {
    id: i32,
    board_id: i32,
    root_id: i32,
    level: i32,
    order_num: i32,
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

    let (mode, board, post, parent) = match (query.board_id, query.post_id, query.reply_to) {
        (Some(board_id), None, None) => {
            ("create", editor_board(&state, board_id).await?, None, None)
        }
        (None, Some(post_id), None) => {
            let post = editor_post(&state, post_id).await?;
            if !may_update_post(&viewer, &post) {
                return Ok(post_error(
                    StatusCode::FORBIDDEN,
                    "not_authorized",
                    "You are not authorized to update this post.",
                ));
            }
            let board = editor_board(&state, post.board_id).await?;
            ("update", board, Some(post), None)
        }
        (None, None, Some(parent_id)) => {
            let parent = editor_post(&state, parent_id).await?;
            if !reply_tree_is_open(&state, parent_id).await? {
                return Ok(reply_closed_error());
            }
            let board = editor_board(&state, parent.board_id).await?;
            ("reply", board, None, Some(parent))
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
        boards: board_navigation(&state).await?,
        post_subject_max_length: state.post_subject_max_length,
        post_content_max_bytes: state.post_content_max_bytes,
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
            let input = match validate_input(&state, &request, request.post_type.unwrap_or(-1)) {
                Ok(input) => input,
                Err(response) => return Ok(response),
            };
            create_post(&state, &viewer, board_id, input).await
        }
        (None, Some(post_id), None) => update_post(&state, &viewer, post_id, &request).await,
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
            reply_to_post(&state, &viewer, parent_id, input).await
        }
        _ => Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_target",
            "Select exactly one board for a new post, one post to update, or one post to reply to.",
        )),
    }
}

pub async fn upload_image(
    Path(post_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
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
            "Login is required to upload an image.",
        ));
    };
    if body.is_empty() || body.len() > state.image_upload_max_bytes {
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_size",
            "The selected image exceeds the configured upload size limit.",
        ));
    }
    let Some((format, original_extension)) = upload_format(&headers, &body) else {
        return Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_image_type",
            "Only JPG, PNG, and GIF images may be uploaded.",
        ));
    };

    let existing = editor_post(&state, post_id).await?;
    if !may_attach_image(&viewer, &existing) {
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to upload an image for this post.",
        ));
    }
    if existing.image_url.is_some() {
        return Ok(post_error(
            StatusCode::CONFLICT,
            "image_update_not_allowed",
            "An attached image cannot be replaced.",
        ));
    }

    let (stored_body, extension, compressed) = if body.len() > IMAGE_COMPRESSION_THRESHOLD_BYTES {
        let source = body.to_vec();
        let compressed_body =
            match tokio::task::spawn_blocking(move || compress_image(&source, format)).await {
                Ok(Ok(compressed_body)) => compressed_body,
                Ok(Err(_)) => {
                    return Ok(post_error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid_image_type",
                        "Only valid JPG, PNG, and GIF images may be uploaded.",
                    ));
                }
                Err(error) => return Err(anyhow::Error::from(error).into()),
            };
        (compressed_body, "jpg", true)
    } else {
        (body.to_vec(), original_extension, false)
    };

    let relative_path = format!("uploads/post-{post_id}.{extension}");
    let upload_directory = state.image_directory.join("uploads");
    tokio::fs::create_dir_all(&upload_directory)
        .await
        .map_err(anyhow::Error::from)?;
    let destination = upload_directory.join(format!("post-{post_id}.{extension}"));
    tokio::fs::write(&destination, &stored_body)
        .await
        .map_err(anyhow::Error::from)?;

    sqlx::query("UPDATE post SET image_url = $1 WHERE id = $2")
        .bind(&relative_path)
        .bind(post_id)
        .execute(&state.pool)
        .await?;
    remove_replaced_upload(&state, post_id, extension).await;
    home::invalidate_cache(&state).await;

    Ok(no_store_json(UploadedImageResponse {
        uploaded: true,
        post_id,
        image_url: relative_path,
        compressed,
        stored_bytes: stored_body.len(),
    }))
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
    let Some((board_id, author_id)) = sqlx::query_as::<_, (i32, Option<i32>)>(
        "SELECT board_id, user_id FROM post WHERE id = $1 AND state IN (0, 1) FOR UPDATE",
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

    if !may_delete_post(&mut transaction, &viewer, board_id).await? {
        transaction.rollback().await?;
        return Ok(post_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to delete this post.",
        ));
    }

    sqlx::query("UPDATE post SET state = 2 WHERE id = $1")
        .bind(post_id)
        .execute(&mut *transaction)
        .await?;
    if let Some(author_id) = author_id {
        refresh_statistics(&mut transaction, board_id, author_id).await?;
    } else {
        refresh_board_statistics(&mut transaction, board_id).await?;
    }
    transaction.commit().await?;
    home::invalidate_cache(&state).await;

    Ok(no_store_json(DeletedPostResponse {
        deleted: true,
        post_id,
        board_id,
    }))
}

async fn create_post(
    state: &AppState,
    viewer: &AuthenticatedUser,
    board_id: i32,
    input: ValidatedPostInput,
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
    let post_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO post (
            subject, board_id, user_id, user_name, post_time, reply_time,
            size, reply_count, access_count, point, type, state, content,
            link_name, link_url, image_url, parent_id, level, order_num
        )
        VALUES (
            $1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
            $5, 1, 0, 0, $6, $7, $8, NULL, NULL, NULL, 0, 0, 0
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
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query("UPDATE post SET root_id = id WHERE id = $1")
        .bind(post_id)
        .execute(&mut *transaction)
        .await?;
    refresh_statistics(&mut transaction, board_id, viewer.id).await?;
    transaction.commit().await?;
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
            id, board_id, level, subject, content, type AS post_type, state,
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
    let update_post_type = if existing.level == 0 {
        request.post_type.unwrap_or(-1)
    } else {
        0
    };
    let input = match validate_input(state, request, update_post_type) {
        Ok(input) => input,
        Err(response) => {
            transaction.rollback().await?;
            return Ok(response);
        }
    };

    sqlx::query(
        r#"
        UPDATE post
        SET subject = $1,
            content = $2,
            size = $3,
            type = $4,
            state = $5
        WHERE id = $6
        "#,
    )
    .bind(input.subject)
    .bind(input.content)
    .bind(input.size)
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
        SELECT id, board_id, COALESCE(root_id, id) AS root_id, level, order_num
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

    sqlx::query("UPDATE post SET order_num = order_num + 1 WHERE root_id = $1 AND order_num > $2")
        .bind(parent.root_id)
        .bind(parent.order_num)
        .execute(&mut *transaction)
        .await?;
    let post_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO post (
            subject, board_id, user_id, user_name, post_time, reply_time,
            size, reply_count, access_count, point, type, state, content,
            link_name, link_url, image_url, parent_id, root_id, level, order_num
        )
        VALUES (
            $1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
            $5, 0, 0, 0, 0, $6, $7, NULL, NULL, NULL, $8, $9, $10, $11
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

fn validate_input(
    state: &AppState,
    request: &SavePostRequest,
    post_type: i32,
) -> Result<ValidatedPostInput, Response> {
    let subject = request.subject.trim().to_string();
    if subject.is_empty() || subject.chars().count() > state.post_subject_max_length {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_subject",
            "Post subject exceeds the configured length limit.",
        ));
    }
    if !matches!(post_type, 0..=3) || !matches!(request.state, 0..=1) {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_post_option",
            "Select a valid post type and visibility.",
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
        post_type,
        state: request.state,
    })
}

fn optional_content(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

async fn editor_post(state: &AppState, post_id: i32) -> AppResult<EditorPost> {
    sqlx::query_as::<_, EditorPost>(
        r#"
        SELECT
            id, board_id, level, subject, content, type AS post_type, state,
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

async fn board_navigation(state: &AppState) -> AppResult<Vec<BoardNavSummary>> {
    Ok(sqlx::query_as::<_, BoardNavSummary>(
        r#"
        SELECT b.id, BTRIM(b.name) AS name, b.category_id, BTRIM(c.name) AS category_name
        FROM board b
        JOIN category c ON c.id = b.category_id
        ORDER BY c.order_id, b.order_id, b.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?)
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
        UPDATE user_info AS u
        SET post_count = (
                SELECT COUNT(*)::integer FROM post p
                WHERE p.user_id = u.id AND p.state IN (0, 1)
            ),
            doc_count = (
                SELECT COUNT(*)::integer FROM post p
                WHERE p.user_id = u.id AND p.type = 1 AND p.state IN (0, 1)
            )
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
        UPDATE board AS b
        SET post_count = (
                SELECT COUNT(*)::integer FROM post p
                WHERE p.board_id = b.id AND p.state IN (0, 1)
            ),
            root_count = (
                SELECT COUNT(*)::integer FROM post p
                WHERE p.board_id = b.id AND p.state IN (0, 1)
                  AND COALESCE(p.parent_id, 0) = 0
            )
        WHERE b.id = $1
        "#,
    )
    .bind(board_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn upload_format(headers: &HeaderMap, body: &[u8]) -> Option<(ImageFormat, &'static str)> {
    match headers.get(header::CONTENT_TYPE)?.to_str().ok()? {
        "image/jpeg" if body.starts_with(&[0xff, 0xd8, 0xff]) => Some((ImageFormat::Jpeg, "jpg")),
        "image/png" if body.starts_with(b"\x89PNG\r\n\x1a\n") => Some((ImageFormat::Png, "png")),
        "image/gif" if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") => {
            Some((ImageFormat::Gif, "gif"))
        }
        _ => None,
    }
}

fn compress_image(bytes: &[u8], format: ImageFormat) -> image::ImageResult<Vec<u8>> {
    let decoded = image::load_from_memory_with_format(bytes, format)?;
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

async fn remove_replaced_upload(state: &AppState, post_id: i32, retained_extension: &str) {
    for extension in ["jpg", "png", "gif"] {
        if extension == retained_extension {
            continue;
        }
        let path = state
            .image_directory
            .join("uploads")
            .join(format!("post-{post_id}.{extension}"));
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(?error, ?path, "failed to remove replaced post image"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    use super::{COMPRESSED_IMAGE_MAX_BYTES, compress_image};

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
}

fn may_update_post(viewer: &AuthenticatedUser, post: &EditorPost) -> bool {
    viewer.level >= ADMIN_LEVEL || (post.level == 0 && post.user_id == Some(viewer.id))
}

fn may_attach_image(viewer: &AuthenticatedUser, post: &EditorPost) -> bool {
    viewer.level >= ADMIN_LEVEL || post.user_id == Some(viewer.id)
}

async fn may_delete_post(
    transaction: &mut Transaction<'_, Postgres>,
    viewer: &AuthenticatedUser,
    board_id: i32,
) -> Result<bool, sqlx::Error> {
    if viewer.level >= ADMIN_LEVEL {
        return Ok(true);
    }

    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM board_master WHERE board_id = $1 AND user_id = $2)",
    )
    .bind(board_id)
    .bind(viewer.id)
    .fetch_one(&mut **transaction)
    .await
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
