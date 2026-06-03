use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::AppResult, routes::auth, state::AppState};

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct PostSearchQuery {
    subject: Option<String>,
    content: Option<String>,
    user_name: Option<String>,
    created_from: Option<String>,
    created_to: Option<String>,
    replied_from: Option<String>,
    replied_to: Option<String>,
    post_type: Option<i32>,
    has_image: Option<bool>,
    has_link: Option<bool>,
    order: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SearchOrder {
    IdDesc,
    IdAsc,
}

impl SearchOrder {
    fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("id_asc") => Self::IdAsc,
            _ => Self::IdDesc,
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::IdDesc => "p.id DESC",
            Self::IdAsc => "p.id ASC",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PostSearchResponse {
    site_name: String,
    filters: NormalizedSearchFilters,
    order: SearchOrder,
    pager: SearchPager,
    posts: Vec<SearchPostSummary>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct NormalizedSearchFilters {
    subject: String,
    content: String,
    user_name: String,
    created_from: String,
    created_to: String,
    replied_from: String,
    replied_to: String,
    post_type: Option<i32>,
    has_image: bool,
    has_link: bool,
}

#[derive(Debug, Serialize)]
pub struct SearchPager {
    page: i64,
    page_size: i64,
    total_pages: i64,
    total_posts: i64,
    has_previous: bool,
    has_next: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SearchPostSummary {
    id: i32,
    root_id: i32,
    parent_id: Option<i32>,
    level: i32,
    subject: Option<String>,
    board_id: Option<i32>,
    board_name: Option<String>,
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

#[derive(Debug, Serialize, FromRow)]
pub struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

pub async fn posts(
    Query(query): Query<PostSearchQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if auth::current_user(&state, &headers).await?.is_none() {
        return Ok(search_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to search posts.",
        ));
    }

    let filters = NormalizedSearchFilters {
        subject: normalize_text(query.subject),
        content: normalize_text(query.content),
        user_name: normalize_text(query.user_name),
        created_from: normalize_text(query.created_from),
        created_to: normalize_text(query.created_to),
        replied_from: normalize_text(query.replied_from),
        replied_to: normalize_text(query.replied_to),
        post_type: query
            .post_type
            .filter(|value| matches!(value, 0 | 1 | 2 | 3)),
        has_image: query.has_image.unwrap_or(false),
        has_link: query.has_link.unwrap_or(false),
    };
    if let Err(response) = validate_date_filters(&filters) {
        return Ok(response);
    }
    let order = SearchOrder::from_query(query.order.as_deref());
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let requested_page = query.page.unwrap_or(1).max(1);
    let total_posts = search_count(&state, &filters).await?;
    let total_pages = total_pages(total_posts, page_size);
    let page = requested_page.min(total_pages.max(1));
    let posts = search_posts(&state, &filters, order, page_size, (page - 1) * page_size).await?;
    let boards = board_navigation(&state).await?;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(PostSearchResponse {
            site_name: state.site_name.clone(),
            filters,
            order,
            pager: SearchPager {
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

async fn search_count(state: &AppState, filters: &NormalizedSearchFilters) -> AppResult<i64> {
    let count = sqlx::query_scalar::<_, i64>(&format!(
        r#"
        SELECT COUNT(*)
        FROM post p
        WHERE {}
        "#,
        search_where_clause()
    ))
    .bind(&filters.subject)
    .bind(&filters.content)
    .bind(&filters.user_name)
    .bind(&filters.created_from)
    .bind(&filters.created_to)
    .bind(&filters.replied_from)
    .bind(&filters.replied_to)
    .bind(filters.post_type)
    .bind(filters.has_image)
    .bind(filters.has_link)
    .fetch_one(&state.pool)
    .await?;

    Ok(count)
}

async fn search_posts(
    state: &AppState,
    filters: &NormalizedSearchFilters,
    order: SearchOrder,
    page_size: i64,
    offset: i64,
) -> AppResult<Vec<SearchPostSummary>> {
    let query = format!(
        r#"
        SELECT
            p.id,
            COALESCE(p.root_id, p.id) AS root_id,
            p.parent_id,
            p.level,
            NULLIF(BTRIM(p.subject), '') AS subject,
            p.board_id,
            NULLIF(BTRIM(b.name), '') AS board_name,
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
            NULLIF(BTRIM(p.link_url), '') AS link_url,
            NULLIF(BTRIM(p.image_url), '') AS image_url
        FROM post p
        LEFT JOIN board b ON b.id = p.board_id
        WHERE {}
        ORDER BY {}
        LIMIT $11 OFFSET $12
        "#,
        search_where_clause(),
        order.sql()
    );
    let posts = sqlx::query_as::<_, SearchPostSummary>(&query)
        .bind(&filters.subject)
        .bind(&filters.content)
        .bind(&filters.user_name)
        .bind(&filters.created_from)
        .bind(&filters.created_to)
        .bind(&filters.replied_from)
        .bind(&filters.replied_to)
        .bind(filters.post_type)
        .bind(filters.has_image)
        .bind(filters.has_link)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    Ok(posts)
}

fn search_where_clause() -> &'static str {
    r#"
        p.state IN (0, 1)
        AND (
               $1 = ''
            OR p.subject ILIKE '%' || REPLACE(REPLACE(REPLACE($1, '\', '\\'), '%', '\%'), '_', '\_') || '%' ESCAPE '\'
            OR to_tsvector('simple', COALESCE(p.subject, '')) @@ plainto_tsquery('simple', $1)
        )
        AND (
               $2 = ''
            OR p.content ILIKE '%' || REPLACE(REPLACE(REPLACE($2, '\', '\\'), '%', '\%'), '_', '\_') || '%' ESCAPE '\'
            OR to_tsvector('simple', COALESCE(p.content, '')) @@ plainto_tsquery('simple', $2)
        )
        AND (
               $3 = ''
            OR p.user_name ILIKE '%' || REPLACE(REPLACE(REPLACE($3, '\', '\\'), '%', '\%'), '_', '\_') || '%' ESCAPE '\'
            OR to_tsvector('simple', COALESCE(p.user_name, '')) @@ plainto_tsquery('simple', $3)
        )
        AND ($4 = '' OR p.post_time >= $4::date)
        AND ($5 = '' OR p.post_time < $5::date + INTERVAL '1 day')
        AND ($6 = '' OR p.reply_time >= $6::date)
        AND ($7 = '' OR p.reply_time < $7::date + INTERVAL '1 day')
        AND ($8::integer IS NULL OR p.type = $8)
        AND (NOT $9::boolean OR NULLIF(BTRIM(p.image_url), '') IS NOT NULL)
        AND (NOT $10::boolean OR NULLIF(BTRIM(p.link_url), '') IS NOT NULL)
    "#
}

fn validate_date_filters(filters: &NormalizedSearchFilters) -> Result<(), Response> {
    for value in [
        filters.created_from.as_str(),
        filters.created_to.as_str(),
        filters.replied_from.as_str(),
        filters.replied_to.as_str(),
    ] {
        if !value.is_empty() && !valid_date(value) {
            return Err(search_error(
                StatusCode::BAD_REQUEST,
                "invalid_search_filter",
                "Date filters must use YYYY-MM-DD.",
            ));
        }
    }

    Ok(())
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }

    let Ok(year) = value[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u32>() else {
        return false;
    };

    if month == 0 || month > 12 || day == 0 {
        return false;
    }

    day <= days_in_month(year, month)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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

fn normalize_text(value: Option<String>) -> String {
    value.unwrap_or_default().trim().to_string()
}

fn total_pages(total_posts: i64, page_size: i64) -> i64 {
    if total_posts == 0 {
        0
    } else {
        (total_posts + page_size - 1) / page_size
    }
}

fn search_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    #[derive(Serialize)]
    struct ErrorBody {
        error: ErrorMessage,
    }

    #[derive(Serialize)]
    struct ErrorMessage {
        code: &'static str,
        message: &'static str,
    }

    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(ErrorBody {
            error: ErrorMessage { code, message },
        }),
    )
        .into_response()
}
