use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

#[derive(Debug, Serialize)]
struct SiteBoard {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    post_count: i32,
    root_count: Option<i32>,
    masters: Vec<SiteMasterUser>,
    order_id: i32,
}

#[derive(Debug, FromRow)]
struct SiteBoardRow {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    post_count: i32,
    root_count: Option<i32>,
    order_id: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct SiteMasterUser {
    id: i32,
    name: String,
}

#[derive(Debug, FromRow)]
struct SiteBoardMasterRow {
    board_id: i32,
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
pub struct CreateCategoryRequest {
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
}

#[derive(Debug, Deserialize)]
pub struct CreateBoardRequest {
    name: String,
    comment: Option<String>,
    category_id: i32,
    order_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct AddBoardMasterRequest {
    user_id: i32,
}

#[derive(Debug, Serialize)]
struct SiteMutationResponse {
    updated: bool,
    target_id: i32,
}

#[derive(Debug, Serialize)]
struct BoardMasterMutationResponse {
    updated: bool,
    board_id: i32,
    master: SiteMasterUser,
}

#[derive(Debug, Serialize)]
struct RecalculatedBoardStatistics {
    updated_boards: u64,
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

pub async fn create_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateCategoryRequest>,
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
    let category_id = sqlx::query_scalar::<_, i32>(
        r#"
        INSERT INTO category (name, comment, order_id, board_count)
        VALUES ($1, $2, $3, 0)
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(optional_text(request.comment))
    .bind(request.order_id)
    .fetch_one(&state.pool)
    .await?;
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        (
            StatusCode::CREATED,
            Json(SiteMutationResponse {
                updated: true,
                target_id: category_id,
            }),
        ),
    )
        .into_response())
}

pub async fn delete_category(
    Path(category_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let deleted = sqlx::query_scalar::<_, i32>(
        r#"
        DELETE FROM category c
        WHERE c.id = $1
          AND NOT EXISTS (SELECT 1 FROM board b WHERE b.category_id = c.id)
        RETURNING c.id
        "#,
    )
    .bind(category_id)
    .fetch_optional(&state.pool)
    .await?;
    if deleted.is_none() {
        if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM category WHERE id = $1)")
            .bind(category_id)
            .fetch_one(&state.pool)
            .await?
        {
            return Ok(site_error(
                StatusCode::CONFLICT,
                "category_not_empty",
                "A category can be deleted only when it contains no boards.",
            ));
        }
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
    .execute(&state.pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    }
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

pub async fn create_board(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateBoardRequest>,
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
    let board_id = sqlx::query_scalar::<_, i32>(
        r#"
        INSERT INTO board (name, comment, category_id, post_count, root_count, order_id)
        VALUES ($1, $2, $3, 0, 0, $4)
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(optional_text(request.comment))
    .bind(request.category_id)
    .bind(request.order_id)
    .fetch_one(&state.pool)
    .await?;
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        (
            StatusCode::CREATED,
            Json(SiteMutationResponse {
                updated: true,
                target_id: board_id,
            }),
        ),
    )
        .into_response())
}

pub async fn delete_board(
    Path(board_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let deleted = sqlx::query_scalar::<_, i32>(
        r#"
        DELETE FROM board b
        WHERE b.id = $1
          AND NOT EXISTS (SELECT 1 FROM post p WHERE p.board_id = b.id)
        RETURNING b.id
        "#,
    )
    .bind(board_id)
    .fetch_optional(&state.pool)
    .await?;
    if deleted.is_none() {
        if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM board WHERE id = $1)")
            .bind(board_id)
            .fetch_one(&state.pool)
            .await?
        {
            return Ok(site_error(
                StatusCode::CONFLICT,
                "board_not_empty",
                "A board can be deleted only when it contains no posts.",
            ));
        }
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    }
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

pub async fn add_board_master(
    Path(board_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddBoardMasterRequest>,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    if !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM board WHERE id = $1)")
        .bind(board_id)
        .fetch_one(&state.pool)
        .await?
    {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "board_not_found",
            "The requested board was not found.",
        ));
    }
    let Some(master) = sqlx::query_as::<_, SiteMasterUser>(
        "SELECT id, BTRIM(name) AS name FROM user_info WHERE id = $1",
    )
    .bind(request.user_id)
    .fetch_optional(&state.pool)
    .await?
    else {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_master",
            "The selected board master does not exist.",
        ));
    };
    let result = sqlx::query(
        r#"
        INSERT INTO board_master (board_id, user_id, order_id)
        SELECT $1, $2, COALESCE(MAX(order_id), 0) + 1
        FROM board_master
        WHERE board_id = $1
        ON CONFLICT (board_id, user_id) DO NOTHING
        "#,
    )
    .bind(board_id)
    .bind(request.user_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(site_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "duplicate_master",
            "This user is already a board master.",
        ));
    }
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(BoardMasterMutationResponse {
            updated: true,
            board_id,
            master,
        }),
    )
        .into_response())
}

pub async fn remove_board_master(
    Path((board_id, user_id)): Path<(i32, i32)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let Some(master) = sqlx::query_as::<_, SiteMasterUser>(
        r#"
        SELECT u.id, BTRIM(u.name) AS name
        FROM board_master bm
        JOIN user_info u ON u.id = bm.user_id
        WHERE bm.board_id = $1 AND bm.user_id = $2
        "#,
    )
    .bind(board_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    else {
        return Ok(site_error(
            StatusCode::NOT_FOUND,
            "master_not_found",
            "This board master assignment was not found.",
        ));
    };
    let mut transaction = state.pool.begin().await?;
    sqlx::query("DELETE FROM board_master WHERE board_id = $1 AND user_id = $2")
        .bind(board_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        r#"
        WITH positions AS (
            SELECT user_id, ROW_NUMBER() OVER (ORDER BY order_id, user_id)::integer AS order_id
            FROM board_master
            WHERE board_id = $1
        )
        UPDATE board_master bm
        SET order_id = positions.order_id
        FROM positions
        WHERE bm.board_id = $1
          AND bm.user_id = positions.user_id
        "#,
    )
    .bind(board_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(BoardMasterMutationResponse {
            updated: true,
            board_id,
            master,
        }),
    )
        .into_response())
}

pub async fn recalculate_board_statistics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if let Some(response) = verified_administrator_mutation(&state, &headers).await? {
        return Ok(response);
    }
    let result = sqlx::query(
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
        "#,
    )
    .execute(&state.pool)
    .await?;
    home::invalidate_cache(&state).await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(RecalculatedBoardStatistics {
            updated_boards: result.rows_affected(),
        }),
    )
        .into_response())
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
    let boards = sqlx::query_as::<_, SiteBoardRow>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            NULLIF(BTRIM(b.comment), '') AS comment,
            b.category_id,
            b.post_count,
            b.root_count,
            b.order_id
        FROM board b
        ORDER BY b.category_id, b.order_id, b.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    let master_rows = sqlx::query_as::<_, SiteBoardMasterRow>(
        r#"
        SELECT bm.board_id, u.id, BTRIM(u.name) AS name
        FROM board_master bm
        JOIN user_info u ON u.id = bm.user_id
        ORDER BY bm.board_id, bm.order_id, u.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(boards
        .into_iter()
        .map(|board| SiteBoard {
            masters: master_rows
                .iter()
                .filter(|master| master.board_id == board.id)
                .map(|master| SiteMasterUser {
                    id: master.id,
                    name: master.name.clone(),
                })
                .collect(),
            id: board.id,
            name: board.name,
            comment: board.comment,
            category_id: board.category_id,
            post_count: board.post_count,
            root_count: board.root_count,
            order_id: board.order_id,
        })
        .collect())
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
