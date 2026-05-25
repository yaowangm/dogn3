use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashSet;

use crate::{
    error::AppResult,
    routes::{auth, home},
    state::AppState,
};

const ADMIN_LEVEL: i32 = 10;

#[derive(Debug, Serialize)]
pub struct SiteManagerResponse {
    site_name: String,
    categories: Vec<SiteCategory>,
    boards: Vec<SiteBoard>,
    master_users: Vec<SiteMasterUser>,
    navigation_boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize, FromRow)]
struct SiteCategory {
    id: i32,
    name: String,
    comment: Option<String>,
    order_id: i32,
    board_count: i64,
}

#[derive(Debug, Serialize, FromRow)]
struct SiteBoard {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    post_count: i32,
    root_count: Option<i32>,
    master_user_ids: Vec<i32>,
    order_id: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct SiteMasterUser {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize, FromRow)]
struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    name: String,
    comment: Option<String>,
    order_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBoardRequest {
    name: String,
    comment: Option<String>,
    category_id: i32,
    order_id: i32,
    master_user_ids: Vec<i32>,
}

#[derive(Debug, Serialize)]
struct SiteMutationResponse {
    updated: bool,
    target_id: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct RecalculatedBoardStatistics {
    board_id: i32,
    post_count: i32,
    root_count: i32,
}

#[derive(Debug, Serialize)]
struct SiteMutationErrorResponse {
    error: SiteMutationError,
}

#[derive(Debug, Serialize)]
struct SiteMutationError {
    code: &'static str,
    message: &'static str,
}

pub async fn manager(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(response) = deny_unless_administrator(&state, &headers).await? {
        return Ok(response);
    }

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(SiteManagerResponse {
            site_name: state.site_name.clone(),
            categories: categories(&state).await?,
            boards: boards(&state).await?,
            master_users: master_users(&state).await?,
            navigation_boards: navigation_boards(&state).await?,
        }),
    )
        .into_response())
}

pub async fn update_category(
    Path(category_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateCategoryRequest>,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let Some(name) = required_name(&request.name) else {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_name",
            "Category name is required.",
        ));
    };
    let result = sqlx::query(
        r#"
        UPDATE category
        SET name = $1, comment = $2, order_id = $3
        WHERE id = $4
        "#,
    )
    .bind(name)
    .bind(optional_text(request.comment))
    .bind(request.order_id)
    .bind(category_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "category_not_found",
            "The requested category was not found.",
        ));
    }
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(SiteMutationResponse {
            updated: true,
            target_id: category_id,
        }),
    )
        .into_response())
}

pub async fn update_board(
    Path(board_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateBoardRequest>,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let Some(name) = required_name(&request.name) else {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_name",
            "Board name is required.",
        ));
    };
    if !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM category WHERE id = $1)")
        .bind(request.category_id)
        .fetch_one(&state.pool)
        .await?
    {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_category",
            "The selected category does not exist.",
        ));
    }
    let unique_master_ids = request
        .master_user_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if unique_master_ids.len() != request.master_user_ids.len() {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "duplicate_master",
            "A board master may only be selected once.",
        ));
    }
    let existing_master_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_info WHERE id = ANY($1::integer[])",
    )
    .bind(&request.master_user_ids)
    .fetch_one(&state.pool)
    .await?;
    if existing_master_count != request.master_user_ids.len() as i64 {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_master",
            "A selected board master does not exist.",
        ));
    }
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE board
        SET name = $1,
            comment = $2,
            category_id = $3,
            order_id = $4
        WHERE id = $5
        "#,
    )
    .bind(name)
    .bind(optional_text(request.comment))
    .bind(request.category_id)
    .bind(request.order_id)
    .bind(board_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    }
    sqlx::query("DELETE FROM board_master WHERE board_id = $1")
        .bind(board_id)
        .execute(&mut *transaction)
        .await?;
    for (position, user_id) in request.master_user_ids.iter().enumerate() {
        sqlx::query("INSERT INTO board_master (board_id, user_id, order_id) VALUES ($1, $2, $3)")
            .bind(board_id)
            .bind(user_id)
            .bind(position as i32 + 1)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(SiteMutationResponse {
            updated: true,
            target_id: board_id,
        }),
    )
        .into_response())
}

pub async fn recalculate_board_statistics(
    Path(board_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let statistics = sqlx::query_as::<_, RecalculatedBoardStatistics>(
        r#"
        UPDATE board AS b
        SET post_count = (
                SELECT COUNT(*)::integer
                FROM post p
                WHERE p.board_id = b.id
                  AND p.state IN (0, 1)
            ),
            root_count = (
                SELECT COUNT(*)::integer
                FROM post p
                WHERE p.board_id = b.id
                  AND p.state IN (0, 1)
                  AND COALESCE(p.parent_id, 0) = 0
            )
        WHERE b.id = $1
        RETURNING b.id AS board_id, b.post_count, COALESCE(b.root_count, 0) AS root_count
        "#,
    )
    .bind(board_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(statistics) = statistics else {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    };
    home::invalidate_cache(&state).await;

    Ok(([(header::CACHE_CONTROL, "no-store")], Json(statistics)).into_response())
}

async fn deny_unless_administrator(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<Option<Response>> {
    let Some(viewer) = auth::current_user(state, headers).await? else {
        return Ok(Some(site_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Administrator login is required to manage the site.",
        )));
    };
    if viewer.level < ADMIN_LEVEL {
        return Ok(Some(site_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "Administrator privilege is required to manage the site.",
        )));
    }
    Ok(None)
}

async fn verified_administrator_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<Option<Response>> {
    if !auth::mutation_request_is_verified(headers) {
        return Ok(Some(site_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        )));
    }
    deny_unless_administrator(state, headers).await
}

async fn categories(state: &AppState) -> AppResult<Vec<SiteCategory>> {
    Ok(sqlx::query_as::<_, SiteCategory>(
        r#"
        SELECT
            c.id,
            BTRIM(c.name) AS name,
            NULLIF(BTRIM(c.comment), '') AS comment,
            c.order_id,
            COUNT(b.id) AS board_count
        FROM category c
        LEFT JOIN board b ON b.category_id = c.id
        GROUP BY c.id, c.name, c.comment, c.order_id
        ORDER BY c.order_id, c.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn boards(state: &AppState) -> AppResult<Vec<SiteBoard>> {
    Ok(sqlx::query_as::<_, SiteBoard>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            NULLIF(BTRIM(b.comment), '') AS comment,
            b.category_id,
            b.post_count,
            b.root_count,
            COALESCE(
                array_agg(bm.user_id ORDER BY bm.order_id) FILTER (WHERE bm.user_id IS NOT NULL),
                ARRAY[]::integer[]
            ) AS master_user_ids,
            b.order_id
        FROM board b
        LEFT JOIN board_master bm ON bm.board_id = b.id
        GROUP BY b.id, b.name, b.comment, b.category_id, b.post_count, b.root_count, b.order_id
        ORDER BY b.category_id, b.order_id, b.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn master_users(state: &AppState) -> AppResult<Vec<SiteMasterUser>> {
    Ok(sqlx::query_as::<_, SiteMasterUser>(
        "SELECT id, BTRIM(name) AS name FROM user_info ORDER BY name, id",
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn navigation_boards(state: &AppState) -> AppResult<Vec<BoardNavSummary>> {
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

fn required_name(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn site_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(SiteMutationErrorResponse {
            error: SiteMutationError { code, message },
        }),
    )
        .into_response()
}
