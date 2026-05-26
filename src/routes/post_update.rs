use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};

use crate::{
    auth::AuthenticatedUser,
    error::AppResult,
    routes::{auth, home},
    state::AppState,
};

const ADMIN_LEVEL: i32 = 10;
const MAX_SUBJECT_LENGTH: usize = 100;
const MAX_LINK_NAME_LENGTH: usize = 25;
const MAX_RESOURCE_LENGTH: usize = 100;

#[derive(Debug, Deserialize)]
pub struct PostEditorQuery {
    board_id: Option<i32>,
    post_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SavePostRequest {
    board_id: Option<i32>,
    post_id: Option<i32>,
    subject: String,
    content: Option<String>,
    post_type: i32,
    state: i32,
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostEditorResponse {
    site_name: String,
    mode: &'static str,
    board: EditorBoard,
    post: Option<EditorPost>,
    boards: Vec<BoardNavSummary>,
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
    subject: Option<String>,
    content: Option<String>,
    post_type: Option<i32>,
    state: i32,
    link_name: Option<String>,
    link_url: Option<String>,
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
    link_name: Option<String>,
    link_url: Option<String>,
    image_url: Option<String>,
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

    let (mode, board, post) = match (query.board_id, query.post_id) {
        (Some(board_id), None) => ("create", editor_board(&state, board_id).await?, None),
        (None, Some(post_id)) => {
            let post = editor_post(&state, post_id).await?;
            if !may_update_post(&viewer, &post) {
                return Ok(post_error(
                    StatusCode::FORBIDDEN,
                    "not_authorized",
                    "You are not authorized to update this post.",
                ));
            }
            let board = editor_board(&state, post.board_id).await?;
            ("update", board, Some(post))
        }
        _ => {
            return Ok(post_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_target",
                "Select exactly one board for a new post or one post to update.",
            ));
        }
    };

    Ok(no_store_json(PostEditorResponse {
        site_name: state.site_name.clone(),
        mode,
        board,
        post,
        boards: board_navigation(&state).await?,
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
    let target = (request.board_id, request.post_id);
    let input = match validate_input(request) {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };

    match target {
        (Some(board_id), None) => create_post(&state, &viewer, board_id, input).await,
        (None, Some(post_id)) => update_post(&state, &viewer, post_id, input).await,
        _ => Ok(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_target",
            "Select exactly one board for a new post or one post to update.",
        )),
    }
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
            $5, 1, 0, 0, $6, $7, $8, $9, $10, $11, 0, 0, 0
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
    .bind(input.link_name)
    .bind(input.link_url)
    .bind(input.image_url)
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
    input: ValidatedPostInput,
) -> AppResult<Response> {
    let mut transaction = state.pool.begin().await?;
    let Some(existing) = sqlx::query_as::<_, EditorPost>(
        r#"
        SELECT
            id, board_id, subject, content, type AS post_type, state,
            link_name, link_url, image_url, user_id
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

    sqlx::query(
        r#"
        UPDATE post
        SET subject = $1,
            content = $2,
            size = $3,
            type = $4,
            state = $5,
            link_name = $6,
            link_url = $7,
            image_url = $8
        WHERE id = $9
        "#,
    )
    .bind(input.subject)
    .bind(input.content)
    .bind(input.size)
    .bind(input.post_type)
    .bind(input.state)
    .bind(input.link_name)
    .bind(input.link_url)
    .bind(input.image_url)
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

fn validate_input(request: SavePostRequest) -> Result<ValidatedPostInput, Response> {
    let subject = request.subject.trim().to_string();
    if subject.is_empty() || subject.chars().count() > MAX_SUBJECT_LENGTH {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_subject",
            "Post subject must contain 1 to 100 characters.",
        ));
    }
    if !matches!(request.post_type, 0..=3) || !matches!(request.state, 0..=1) {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_post_option",
            "Select a valid post type and visibility.",
        ));
    }
    let content = optional_content(request.content);
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
    let link_name =
        limited_optional_text(request.link_name, MAX_LINK_NAME_LENGTH, "invalid_link_name")?;
    let link_url =
        limited_optional_text(request.link_url, MAX_RESOURCE_LENGTH, "invalid_link_url")?;
    let image_url =
        limited_optional_text(request.image_url, MAX_RESOURCE_LENGTH, "invalid_image_url")?;

    Ok(ValidatedPostInput {
        subject,
        content,
        size,
        post_type: request.post_type,
        state: request.state,
        link_name,
        link_url,
        image_url,
    })
}

fn optional_content(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn limited_optional_text(
    value: Option<String>,
    max_length: usize,
    code: &'static str,
) -> Result<Option<String>, Response> {
    let value = value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });
    if value
        .as_ref()
        .is_some_and(|value| value.chars().count() > max_length)
    {
        return Err(post_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            "One of the post resource fields is too long.",
        ));
    }
    Ok(value)
}

async fn editor_post(state: &AppState, post_id: i32) -> AppResult<EditorPost> {
    sqlx::query_as::<_, EditorPost>(
        r#"
        SELECT
            id, board_id, subject, content, type AS post_type, state,
            link_name, link_url, image_url, user_id
        FROM post
        WHERE id = $1 AND state IN (0, 1)
        "#,
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(crate::error::AppError::NotFound)
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

fn may_update_post(viewer: &AuthenticatedUser, post: &EditorPost) -> bool {
    viewer.level >= ADMIN_LEVEL || post.user_id == Some(viewer.id)
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
