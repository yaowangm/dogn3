use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use sqlx::FromRow;

use crate::state::AppState;

const INDEX_TEMPLATE: &str = include_str!("../../static/index.html");
const PRINT_TEMPLATE: &str = include_str!("../../static/post_print.html");
const SITE_ICON_PATH: &str = "/assets/favicon.svg";
const ASSET_VERSION: &str = env!("DOGN_ASSET_VERSION");

#[derive(Debug, FromRow)]
struct BoardMetaRow {
    name: String,
    comment: Option<String>,
    category_name: String,
}

#[derive(Debug, FromRow)]
struct PostMetaRow {
    subject: Option<String>,
    content: Option<String>,
    state: i32,
    board_name: String,
    user_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct UserMetaRow {
    name: String,
    intro: Option<String>,
    post_count: i32,
    doc_count: Option<i32>,
    point: Option<i32>,
}

#[derive(Clone, Debug)]
struct PageMeta {
    og_type: &'static str,
    title: String,
    description: String,
}

pub async fn index(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    shell_response(
        render_shell(INDEX_TEMPLATE, &state, uri.path()).await,
        &headers,
    )
}

pub async fn print(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    shell_response(
        render_shell(PRINT_TEMPLATE, &state, uri.path()).await,
        &headers,
    )
}

async fn render_shell(template: &str, state: &AppState, path: &str) -> String {
    let meta = page_meta(state, path)
        .await
        .unwrap_or_else(|| default_meta(state));
    let title = page_title(&meta, state);
    let head = meta_tags(
        &meta,
        &title,
        &canonical_url(state, path),
        &site_icon_url(state),
        &state.site_name,
    );
    template
        .replace("{{ASSET_VERSION}}", ASSET_VERSION)
        .replace(
            "<title>Dogn</title>",
            &format!("<title>{}</title>", escape_html(&title)),
        )
        .replace(
            "<title>Print post</title>",
            &format!("<title>{}</title>", escape_html(&title)),
        )
        .replacen("</head>", &format!("{head}\n  </head>"), 1)
}

fn shell_response(body: String, request_headers: &HeaderMap) -> Response {
    let etag = format!("W/\"{:016x}\"", fnv1a(body.as_bytes()));
    if request_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
    {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        set_shell_cache_headers(response.headers_mut(), &etag);
        return response;
    }

    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    set_shell_cache_headers(response.headers_mut(), &etag);
    response
}

fn set_shell_cache_headers(headers: &mut HeaderMap, etag: &str) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(etag).expect("generated ETag is a valid header"),
    );
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

async fn page_meta(state: &AppState, path: &str) -> Option<PageMeta> {
    match path_segments(path).as_slice() {
        [] => Some(default_meta(state)),
        ["board", board_id] => board_meta(state, parse_id(board_id)?).await,
        ["post", post_id] | ["post_list", post_id] | ["post_print", post_id] => {
            post_meta(state, parse_id(post_id)?).await
        }
        ["user", user_id] => user_meta(state, parse_id(user_id)?).await,
        ["login"] => Some(PageMeta {
            og_type: "website",
            title: "Login".to_string(),
            description: format!("Login to {}.", state.site_name),
        }),
        ["search"] => Some(PageMeta {
            og_type: "website",
            title: "Search".to_string(),
            description: format!("Search posts on {}.", state.site_name),
        }),
        _ => Some(default_meta(state)),
    }
}

async fn board_meta(state: &AppState, board_id: i32) -> Option<PageMeta> {
    let row = sqlx::query_as::<_, BoardMetaRow>(
        r#"
        SELECT
            BTRIM(b.name) AS name,
            NULLIF(BTRIM(b.comment), '') AS comment,
            BTRIM(c.name) AS category_name
        FROM board b
        JOIN category c ON c.id = b.category_id
        WHERE b.id = $1
        "#,
    )
    .bind(board_id)
    .fetch_optional(&state.pool)
    .await
    .ok()??;

    Some(PageMeta {
        og_type: "website",
        title: row.name.clone(),
        description: truncate_description(
            row.comment
                .unwrap_or_else(|| format!("{} board in {}.", row.name, row.category_name)),
        ),
    })
}

async fn post_meta(state: &AppState, post_id: i32) -> Option<PageMeta> {
    let row = sqlx::query_as::<_, PostMetaRow>(
        r#"
        SELECT
            NULLIF(BTRIM(p.subject), '') AS subject,
            NULLIF(p.content, '') AS content,
            p.state,
            BTRIM(b.name) AS board_name,
            NULLIF(BTRIM(p.user_name), '') AS user_name
        FROM post p
        JOIN board b ON b.id = p.board_id
        WHERE p.id = $1
          AND p.state IN (0, 1)
        "#,
    )
    .bind(post_id)
    .fetch_optional(&state.pool)
    .await
    .ok()??;

    let title = row.subject.unwrap_or_else(|| "(untitled)".to_string());
    let fallback = format!(
        "{} posted in {}.",
        row.user_name.unwrap_or_else(|| "A user".to_string()),
        row.board_name
    );
    let description = if row.state == 1 {
        format!("Encrypted post metadata for {title}.")
    } else {
        row.content
            .map(strip_markup)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback)
    };

    Some(PageMeta {
        og_type: "article",
        title,
        description: truncate_description(description),
    })
}

async fn user_meta(state: &AppState, user_id: i32) -> Option<PageMeta> {
    let row = sqlx::query_as::<_, UserMetaRow>(
        r#"
        SELECT
            BTRIM(name) AS name,
            NULLIF(BTRIM(intro), '') AS intro,
            post_count,
            doc_count,
            point
        FROM user_info
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .ok()??;

    let description = row.intro.unwrap_or_else(|| {
        format!(
            "{} has {} posts, {} original posts, and {} points.",
            row.name,
            row.post_count,
            row.doc_count.unwrap_or(0),
            row.point.unwrap_or(0)
        )
    });

    Some(PageMeta {
        og_type: "profile",
        title: row.name,
        description: truncate_description(description),
    })
}

fn default_meta(state: &AppState) -> PageMeta {
    PageMeta {
        og_type: "website",
        title: state.site_name.clone(),
        description: format!("{} forum.", state.site_name),
    }
}

fn page_title(meta: &PageMeta, state: &AppState) -> String {
    if meta.title == state.site_name {
        state.site_name.clone()
    } else {
        format!("{} - {}", meta.title, state.site_name)
    }
}

fn meta_tags(meta: &PageMeta, title: &str, url: &str, image_url: &str, site_name: &str) -> String {
    let description = truncate_description(meta.description.clone());
    format!(
        r#"    <meta name="description" content="{description}">
    <link rel="canonical" href="{url}">
    <meta property="og:type" content="{og_type}">
    <meta property="og:title" content="{title}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="{url}">
    <meta property="og:image" content="{image_url}">
    <meta property="og:site_name" content="{site_name}">"#,
        description = escape_html(&description),
        url = escape_html(url),
        og_type = escape_html(meta.og_type),
        title = escape_html(title),
        image_url = escape_html(image_url),
        site_name = escape_html(site_name),
    )
}

fn canonical_url(state: &AppState, path: &str) -> String {
    match public_site_url(state) {
        Some(base) => format!("{base}{path}"),
        None => path.to_string(),
    }
}

fn site_icon_url(state: &AppState) -> String {
    let path = format!("{SITE_ICON_PATH}?v={ASSET_VERSION}");
    match public_site_url(state) {
        Some(base) => format!("{base}{path}"),
        None => path,
    }
}

fn public_site_url(state: &AppState) -> Option<String> {
    state
        .password_reset
        .public_site_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn parse_id(value: &str) -> Option<i32> {
    value.parse().ok()
}

fn truncate_description(value: String) -> String {
    const MAX_CHARS: usize = 180;
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn strip_markup(value: String) -> String {
    value
        .replace(['#', '*', '`', '>', '|'], " ")
        .replace(['[', ']', '(', ')'], " ")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
