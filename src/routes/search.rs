use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, QueryBuilder};
use std::time::Instant;

use crate::{
    error::AppResult,
    routes::{auth, navigation},
    state::AppState,
};
use navigation::BoardNavSummary;

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
    search_performed: bool,
    search_method: Option<SearchMethod>,
    order: SearchOrder,
    pager: SearchPager,
    posts: Vec<SearchPostSummary>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct SearchMethod {
    name: &'static str,
    search_time_ms: u64,
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
    #[serde(skip)]
    total_posts: i64,
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
    if !filters.has_conditions() {
        let boards = navigation::boards(&state).await?;
        return Ok((
            [(header::CACHE_CONTROL, "no-store")],
            Json(PostSearchResponse {
                site_name: state.site_name.clone(),
                filters,
                search_performed: false,
                search_method: None,
                order,
                pager: SearchPager {
                    page: 1,
                    page_size,
                    total_pages: 0,
                    total_posts: 0,
                    has_previous: false,
                    has_next: false,
                },
                posts: Vec::new(),
                boards,
            }),
        )
            .into_response());
    }

    let search_started_at = Instant::now();
    let (mut total_posts, mut posts) = search_posts(
        &state,
        &filters,
        order,
        page_size,
        (requested_page - 1) * page_size,
    )
    .await?;
    if posts.is_empty() {
        total_posts = search_count(&state, &filters).await?;
    }
    let total_pages = total_pages(total_posts, page_size);
    let page = requested_page.min(total_pages.max(1));
    if total_posts > 0 && page != requested_page {
        (_, posts) =
            search_posts(&state, &filters, order, page_size, (page - 1) * page_size).await?;
    }
    let search_time_ms = elapsed_millis(search_started_at);
    let boards = navigation::boards(&state).await?;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(PostSearchResponse {
            site_name: state.site_name.clone(),
            filters,
            search_performed: true,
            search_method: Some(SearchMethod::current(search_time_ms)),
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

impl NormalizedSearchFilters {
    fn has_conditions(&self) -> bool {
        !self.subject.is_empty()
            || !self.content.is_empty()
            || !self.user_name.is_empty()
            || !self.created_from.is_empty()
            || !self.created_to.is_empty()
            || !self.replied_from.is_empty()
            || !self.replied_to.is_empty()
            || self.post_type.is_some()
            || self.has_image
            || self.has_link
    }
}

impl SearchMethod {
    fn current(search_time_ms: u64) -> Self {
        Self {
            name: "PGroonga Chinese/multilingual full-text search",
            search_time_ms,
        }
    }
}

fn elapsed_millis(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

async fn search_count(state: &AppState, filters: &NormalizedSearchFilters) -> AppResult<i64> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        SELECT COUNT(*)
        FROM post p
        JOIN board b ON b.id = p.board_id
        WHERE
        "#,
    );
    push_search_filters(&mut query, filters);
    let count = query
        .build_query_scalar::<i64>()
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
) -> AppResult<(i64, Vec<SearchPostSummary>)> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        SELECT
            COUNT(*) OVER() AS total_posts,
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
        JOIN board b ON b.id = p.board_id
        WHERE
        "#,
    );
    push_search_filters(&mut query, filters);
    query
        .push(" ORDER BY ")
        .push(order.sql())
        .push(" LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind(offset);
    let posts = query
        .build_query_as::<SearchPostSummary>()
        .fetch_all(&state.pool)
        .await?;

    let total_posts = posts.first().map_or(0, |post| post.total_posts);
    Ok((total_posts, posts))
}

fn push_search_filters<'a>(
    query: &mut QueryBuilder<'a, Postgres>,
    filters: &'a NormalizedSearchFilters,
) {
    query.push("p.state IN (0, 1)");
    if !filters.subject.is_empty() {
        query
            .push(" AND COALESCE(p.subject, '')::text &@ ")
            .push_bind(&filters.subject);
    }
    if !filters.content.is_empty() {
        query
            .push(" AND COALESCE(p.content, '')::text &@ ")
            .push_bind(&filters.content);
    }
    if !filters.user_name.is_empty() {
        query
            .push(" AND COALESCE(p.user_name, '')::text &@ ")
            .push_bind(&filters.user_name);
    }
    if !filters.created_from.is_empty() {
        query
            .push(" AND p.post_time >= ")
            .push_bind(&filters.created_from)
            .push("::date");
    }
    if !filters.created_to.is_empty() {
        query
            .push(" AND p.post_time < ")
            .push_bind(&filters.created_to)
            .push("::date + INTERVAL '1 day'");
    }
    if !filters.replied_from.is_empty() {
        query
            .push(" AND p.reply_time >= ")
            .push_bind(&filters.replied_from)
            .push("::date");
    }
    if !filters.replied_to.is_empty() {
        query
            .push(" AND p.reply_time < ")
            .push_bind(&filters.replied_to)
            .push("::date + INTERVAL '1 day'");
    }
    if let Some(post_type) = filters.post_type {
        query.push(" AND p.type = ").push_bind(post_type);
    }
    if filters.has_image {
        query.push(" AND NULLIF(BTRIM(p.image_url), '') IS NOT NULL");
    }
    if filters.has_link {
        query.push(" AND NULLIF(BTRIM(p.link_url), '') IS NOT NULL");
    }
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

#[cfg(test)]
mod tests {
    use sqlx::{Postgres, QueryBuilder};

    use super::{NormalizedSearchFilters, push_search_filters};

    fn filters() -> NormalizedSearchFilters {
        NormalizedSearchFilters {
            subject: String::new(),
            content: String::new(),
            user_name: String::new(),
            created_from: String::new(),
            created_to: String::new(),
            replied_from: String::new(),
            replied_to: String::new(),
            post_type: None,
            has_image: false,
            has_link: false,
        }
    }

    #[test]
    fn search_sql_contains_only_active_filters() {
        let mut filters = filters();
        filters.subject = "数据库".to_string();
        filters.created_from = "2026-01-01".to_string();
        filters.post_type = Some(1);
        filters.has_image = true;
        let mut query = QueryBuilder::<Postgres>::new("SELECT 1 FROM post p WHERE ");

        push_search_filters(&mut query, &filters);
        let sql = query.sql();

        assert!(sql.contains("COALESCE(p.subject, '')::text &@ $1"));
        assert!(sql.contains("p.post_time >= $2::date"));
        assert!(sql.contains("p.type = $3"));
        assert!(sql.contains("NULLIF(BTRIM(p.image_url), '') IS NOT NULL"));
        assert!(!sql.contains("p.content"));
        assert!(!sql.contains("p.user_name"));
        assert!(!sql.contains("p.reply_time"));
        assert!(!sql.contains("p.link_url"));
        assert!(!sql.contains(" OR "));
    }

    #[test]
    fn search_sql_without_optional_filters_uses_visibility_only() {
        let filters = filters();
        let mut query = QueryBuilder::<Postgres>::new("SELECT 1 FROM post p WHERE ");

        push_search_filters(&mut query, &filters);

        assert_eq!(query.sql(), "SELECT 1 FROM post p WHERE p.state IN (0, 1)");
    }
}
