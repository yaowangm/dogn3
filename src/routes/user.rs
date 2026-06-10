use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    auth::{MODERN_PASSWORD_SCHEME, hash_modern_password},
    error::{AppError, AppResult},
    routes::{auth, home},
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

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    query: Option<String>,
    role: Option<String>,
    order: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    name: String,
    email: Option<String>,
    intro: Option<String>,
    intro_user_id: Option<i32>,
    password: String,
    confirm_password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetRoleRequest {
    level: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    email: Option<String>,
    intro: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActivityKind {
    Original,
    Favorites,
    Signatures,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum UserListOrder {
    IdDesc,
    IdAsc,
}

impl UserListOrder {
    fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("id_asc") => Self::IdAsc,
            _ => Self::IdDesc,
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::IdDesc => "u.id DESC",
            Self::IdAsc => "u.id ASC",
        }
    }
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
    managed_boards: Vec<ManagedBoard>,
    latest_signature: Option<UserSignature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_details: Option<UserPrivateDetails>,
    can_update: bool,
    can_set_role: bool,
    activity: ActivityKind,
    pager: Pager,
    posts: Vec<ActivityPost>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct UserListResponse {
    site_name: String,
    query: String,
    role: Option<i32>,
    active: bool,
    order: UserListOrder,
    pager: UserListPager,
    users: Vec<UserListItem>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize, FromRow)]
struct UserProfile {
    id: i32,
    name: String,
    level: i32,
    reg_time: Option<String>,
    post_count: i32,
    doc_count: Option<i32>,
    last_login: Option<String>,
    point: Option<i32>,
    intro: Option<String>,
    favorite_count: Option<i32>,
}

#[derive(Debug, Serialize, FromRow)]
struct ManagedBoard {
    id: i32,
    name: String,
    category_name: String,
}

#[derive(Debug, Serialize, FromRow)]
struct UserListItem {
    id: i32,
    name: String,
    level: i32,
    email: Option<String>,
    reg_time: Option<String>,
    post_count: i32,
    doc_count: Option<i32>,
    point: Option<i32>,
    favorite_count: Option<i32>,
    last_login: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct UserSignature {
    id: i32,
    content: String,
}

#[derive(Debug, Serialize, FromRow)]
struct UserPrivateDetails {
    email: Option<String>,
    last_login_ip: Option<String>,
    intro_user_id: Option<i32>,
    intro_user_name: Option<String>,
    login_count: Option<i32>,
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

#[derive(Debug, Serialize)]
struct UserListPager {
    page: i64,
    page_size: i64,
    total_pages: i64,
    total_users: i64,
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

#[derive(Debug, Serialize, FromRow)]
struct RecalculatedStatistics {
    user_id: i32,
    post_count: i32,
    doc_count: i32,
    favorite_count: i32,
}

#[derive(Debug, Serialize)]
struct CreatedUserResponse {
    created: bool,
    user_id: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct UpdatedRoleResponse {
    user_id: i32,
    level: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct UpdatedProfileResponse {
    user_id: i32,
    email: Option<String>,
    intro: Option<String>,
}

#[derive(Debug, Serialize)]
struct UserMutationErrorResponse {
    error: UserMutationError,
}

#[derive(Debug, Serialize)]
struct UserMutationError {
    code: &'static str,
    message: &'static str,
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
    let viewer = auth::current_user(&state, &headers).await?;
    let can_read_encrypted = viewer.is_some();
    let can_update = viewer
        .as_ref()
        .is_some_and(|viewer| viewer.id == user_id || viewer.level >= ADMIN_LEVEL);
    let can_set_role = viewer
        .as_ref()
        .is_some_and(|viewer| viewer.level >= ADMIN_LEVEL);
    let managed_boards = managed_boards(&state, user_id).await?;
    let latest_signature = latest_signature(&state, user_id).await?;
    let private_details = if can_update {
        Some(private_details(&state, user_id).await?)
    } else {
        None
    };
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
            managed_boards,
            latest_signature,
            private_details,
            can_update,
            can_set_role,
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

pub async fn user_list(
    Query(query): Query<UserListQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(mutation_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Administrator login is required to view the user list.",
        ));
    };
    if viewer.level < ADMIN_LEVEL {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "Administrator privilege is required to view the user list.",
        ));
    }
    let search = query.query.as_deref().unwrap_or("").trim().to_string();
    let role = query
        .role
        .as_deref()
        .and_then(|role| role.parse::<i32>().ok())
        .filter(|role| matches!(role, 0 | 1 | 5 | 10));
    let active = matches!(query.role.as_deref(), Some("active"));
    let order = UserListOrder::from_query(query.order.as_deref());
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let requested_page = query.page.unwrap_or(1).max(1);
    let total_users: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM user_info u
        WHERE (
                $1 = ''
             OR POSITION(LOWER($1) IN LOWER(BTRIM(u.name))) > 0
             OR POSITION(LOWER($1) IN LOWER(COALESCE(BTRIM(u.email), ''))) > 0
        )
          AND (
                ($3::boolean AND u.level <> 0)
             OR (NOT $3::boolean AND ($2::integer IS NULL OR u.level = $2))
          )
        "#,
    )
    .bind(&search)
    .bind(role)
    .bind(active)
    .fetch_one(&state.pool)
    .await?;
    let total_pages = total_pages(total_users, page_size);
    let page = requested_page.min(total_pages.max(1));
    let list_query = format!(
        r#"
        SELECT
            u.id,
            BTRIM(u.name) AS name,
            u.level,
            NULLIF(BTRIM(u.email), '') AS email,
            to_char(u.reg_time, 'YYYY-MM-DD') AS reg_time,
            u.post_count,
            u.doc_count,
            u.point,
            u.favorite_count,
            to_char(u.last_login, 'YYYY-MM-DD HH24:MI') AS last_login
        FROM user_info u
        WHERE (
                $1 = ''
             OR POSITION(LOWER($1) IN LOWER(BTRIM(u.name))) > 0
             OR POSITION(LOWER($1) IN LOWER(COALESCE(BTRIM(u.email), ''))) > 0
        )
          AND (
                ($5::boolean AND u.level <> 0)
             OR (NOT $5::boolean AND ($2::integer IS NULL OR u.level = $2))
          )
        ORDER BY {}
        LIMIT $3 OFFSET $4
        "#,
        order.sql()
    );
    let users = sqlx::query_as::<_, UserListItem>(&list_query)
        .bind(&search)
        .bind(role)
        .bind(page_size)
        .bind((page - 1) * page_size)
        .bind(active)
        .fetch_all(&state.pool)
        .await?;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(UserListResponse {
            site_name: state.site_name.clone(),
            query: search,
            role,
            active,
            order,
            pager: UserListPager {
                page,
                page_size,
                total_pages,
                total_users,
                has_previous: page > 1,
                has_next: total_pages > 0 && page < total_pages,
            },
            users,
            boards: board_navigation(&state).await?,
        }),
    )
        .into_response())
}

pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateUserRequest>,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }

    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(mutation_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Administrator login is required to add a user.",
        ));
    };
    if viewer.level < ADMIN_LEVEL {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "Administrator privilege is required to add a user.",
        ));
    }

    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 25 {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_name",
            "User name must contain 1 to 25 characters.",
        ));
    }
    let email = request
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty());
    if email.is_some_and(|email| email.chars().count() > 25) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_email",
            "Email address must contain at most 25 characters.",
        ));
    }
    let intro = request
        .intro
        .as_deref()
        .map(str::trim)
        .filter(|intro| !intro.is_empty());
    if intro.is_some_and(|intro| intro.chars().count() > 100) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_intro",
            "Introduction must contain at most 100 characters.",
        ));
    }
    if request.password != request.confirm_password {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "password_confirmation_mismatch",
            "The password confirmation does not match.",
        ));
    }
    if let Err(message) = auth::validate_new_password(&request.password) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_new_password",
            message,
        ));
    }

    let _permit = match state.login_hash_permits.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return Ok(mutation_error(
                StatusCode::TOO_MANY_REQUESTS,
                "password_hash_capacity_exceeded",
                "The server is busy processing credentials. Try again.",
            ));
        }
    };
    let password = hash_modern_password(&request.password)?;
    let mut transaction = state.pool.begin().await?;
    sqlx::query("LOCK TABLE user_info IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *transaction)
        .await?;
    let duplicate: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_info WHERE BTRIM(name) = $1)")
            .bind(name)
            .fetch_one(&mut *transaction)
            .await?;
    if duplicate {
        transaction.rollback().await?;
        return Ok(mutation_error(
            StatusCode::CONFLICT,
            "duplicate_user_name",
            "This user name is already in use.",
        ));
    }
    if let Some(intro_user_id) = request.intro_user_id {
        let introducer_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_info WHERE id = $1)")
                .bind(intro_user_id)
                .fetch_one(&mut *transaction)
                .await?;
        if !introducer_exists {
            transaction.rollback().await?;
            return Ok(mutation_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_intro_user",
                "Select an existing introducing user.",
            ));
        }
    }

    let user_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO user_info (
            name,
            password,
            password_scheme,
            state,
            level,
            email,
            intro,
            intro_user_id,
            reg_time,
            post_count,
            doc_count,
            login_count,
            point,
            favorite_count
        )
        VALUES ($1, $2, $3, 0, 1, $4, $5, $6, CURRENT_TIMESTAMP, 0, 0, 0, $7, 0)
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(password)
    .bind(MODERN_PASSWORD_SCHEME)
    .bind(email)
    .bind(intro)
    .bind(request.intro_user_id)
    .bind(state.new_user_initial_points)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    home::invalidate_cache(&state).await;

    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(CreatedUserResponse {
            created: true,
            user_id,
        }),
    )
        .into_response())
}

pub async fn recalculate_statistics(
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }

    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(mutation_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to recalculate statistics.",
        ));
    };
    if viewer.id != user_id && viewer.level < ADMIN_LEVEL {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to recalculate these statistics.",
        ));
    }

    let statistics = sqlx::query_as::<_, RecalculatedStatistics>(
        r#"
        UPDATE user_info AS u
        SET
            post_count = (
                SELECT COUNT(*)::integer
                FROM post p
                WHERE p.user_id = u.id
                  AND p.state IN (0, 1)
            ),
            doc_count = (
                SELECT COUNT(*)::integer
                FROM post p
                WHERE p.user_id = u.id
                  AND p.type = 1
                  AND p.state IN (0, 1)
            ),
            last_post = (
                SELECT MAX(p.post_time)
                FROM post p
                WHERE p.user_id = u.id
                  AND p.state IN (0, 1)
            ),
            last_origin = (
                SELECT MAX(p.post_time)
                FROM post p
                WHERE p.user_id = u.id
                  AND p.type = 1
                  AND p.state IN (0, 1)
            ),
            last_reship = (
                SELECT MAX(p.post_time)
                FROM post p
                WHERE p.user_id = u.id
                  AND p.type = 2
                  AND p.state IN (0, 1)
            ),
            favorite_count = (
                SELECT COUNT(*)::integer
                FROM favorite f
                JOIN post p ON p.id = f.post_id
                WHERE f.user_id = u.id
                  AND p.state IN (0, 1)
            )
        WHERE u.id = $1
        RETURNING
            u.id AS user_id,
            u.post_count,
            u.doc_count,
            u.favorite_count
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    home::invalidate_cache(&state).await;

    Ok(([(header::CACHE_CONTROL, "no-store")], Json(statistics)).into_response())
}

pub async fn update_profile(
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(mutation_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to update profile information.",
        ));
    };
    if viewer.id != user_id && viewer.level < ADMIN_LEVEL {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to update this profile.",
        ));
    }

    let email = request
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty());
    if email.is_some_and(|email| email.chars().count() > 25) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_email",
            "Email address must contain at most 25 characters.",
        ));
    }
    let intro = request
        .intro
        .as_deref()
        .map(str::trim)
        .filter(|intro| !intro.is_empty());
    if intro.is_some_and(|intro| intro.chars().count() > 100) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_intro",
            "Introduction must contain at most 100 characters.",
        ));
    }

    let updated = sqlx::query_as::<_, UpdatedProfileResponse>(
        r#"
        UPDATE user_info
        SET email = $2, intro = $3
        WHERE id = $1
        RETURNING id AS user_id, NULLIF(BTRIM(email), '') AS email, NULLIF(BTRIM(intro), '') AS intro
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind(intro)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(([(header::CACHE_CONTROL, "no-store")], Json(updated)).into_response())
}

pub async fn set_role(
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetRoleRequest>,
) -> AppResult<Response> {
    if !auth::mutation_request_is_verified(&headers) {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = auth::current_user(&state, &headers).await? else {
        return Ok(mutation_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Administrator login is required to set a role.",
        ));
    };
    if viewer.level < ADMIN_LEVEL {
        return Ok(mutation_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "Administrator privilege is required to set a role.",
        ));
    }
    if !matches!(request.level, 0 | 1 | 10) {
        return Ok(mutation_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_role",
            "Select frozen, member, or administrator.",
        ));
    }

    let mut transaction = state.pool.begin().await?;
    let previous_level: i32 =
        sqlx::query_scalar("SELECT level FROM user_info WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(AppError::NotFound)?;
    let role = sqlx::query_as::<_, UpdatedRoleResponse>(
        r#"
        UPDATE user_info u
        SET level = CASE
            WHEN $2 = 1
             AND EXISTS (SELECT 1 FROM board_master bm WHERE bm.user_id = u.id)
                THEN 5
            ELSE $2
        END
        WHERE u.id = $1
        RETURNING u.id AS user_id, u.level
        "#,
    )
    .bind(user_id)
    .bind(request.level)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    if previous_level != role.level {
        state.sessions.remove_user(user_id);
    }
    home::invalidate_cache(&state).await;

    Ok(([(header::CACHE_CONTROL, "no-store")], Json(role)).into_response())
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
            doc_count,
            to_char(last_login, 'YYYY-MM-DD HH24:MI') AS last_login,
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

async fn managed_boards(state: &AppState, user_id: i32) -> AppResult<Vec<ManagedBoard>> {
    Ok(sqlx::query_as::<_, ManagedBoard>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            BTRIM(c.name) AS category_name
        FROM board_master bm
        JOIN board b ON b.id = bm.board_id
        JOIN category c ON c.id = b.category_id
        WHERE bm.user_id = $1
        ORDER BY c.order_id, c.id, b.order_id, b.id
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?)
}

async fn latest_signature(state: &AppState, user_id: i32) -> AppResult<Option<UserSignature>> {
    Ok(sqlx::query_as::<_, UserSignature>(
        r#"
        SELECT p.id, p.content
        FROM (
            SELECT sign_id
            FROM sign_log
            WHERE user_id = $1
            ORDER BY id DESC
            LIMIT 1
        ) latest
        JOIN post p ON p.id = latest.sign_id
        WHERE p.state = 0
          AND NULLIF(p.content, '') IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?)
}

async fn private_details(state: &AppState, user_id: i32) -> AppResult<UserPrivateDetails> {
    Ok(sqlx::query_as::<_, UserPrivateDetails>(
        r#"
        SELECT
            NULLIF(BTRIM(u.email), '') AS email,
            NULLIF(BTRIM(u.last_login_ip), '') AS last_login_ip,
            introducer.id AS intro_user_id,
            NULLIF(BTRIM(introducer.name), '') AS intro_user_name,
            u.login_count
        FROM user_info u
        LEFT JOIN user_info introducer ON introducer.id = u.intro_user_id
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?)
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

fn mutation_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(UserMutationErrorResponse {
            error: UserMutationError { code, message },
        }),
    )
        .into_response()
}
