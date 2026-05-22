use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::AppResult, state::AppState};

const DEFAULT_PAGE_SIZE: i64 = 10;
const MAX_PAGE_SIZE: i64 = 50;

#[derive(Debug, Deserialize)]
pub struct BoardQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BoardResponse {
    site_name: String,
    board: BoardInfo,
    pager: Pager,
    trees: Vec<PostTree>,
    boards: Vec<BoardNavSummary>,
}

#[derive(Debug, Serialize)]
pub struct BoardInfo {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    category_name: String,
    post_count: i32,
    root_count: i32,
    master_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Pager {
    page: i64,
    page_size: i64,
    total_pages: i64,
    total_roots: i64,
    has_previous: bool,
    has_next: bool,
}

#[derive(Debug, Serialize)]
pub struct PostTree {
    root_id: i32,
    posts: Vec<BoardPostSummary>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardNavSummary {
    id: i32,
    name: String,
    category_id: i32,
    category_name: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BoardPostSummary {
    id: i32,
    root_id: i32,
    parent_id: Option<i32>,
    level: i32,
    subject: Option<String>,
    user_id: Option<i32>,
    user_name: Option<String>,
    post_time: Option<String>,
    reply_time: Option<String>,
    reply_count: Option<i32>,
    access_count: i32,
    point: Option<i32>,
    post_type: Option<i32>,
    state: i32,
    image_url: Option<String>,
}

#[derive(Debug, FromRow)]
struct BoardInfoRow {
    id: i32,
    name: String,
    comment: Option<String>,
    category_id: i32,
    category_name: String,
    post_count: i32,
    root_count: Option<i32>,
    master_name: Option<String>,
    master_name_2: Option<String>,
    master_name_3: Option<String>,
    master_name_4: Option<String>,
}

pub async fn board(
    Path(board_id): Path<i32>,
    Query(query): Query<BoardQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<BoardResponse>> {
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let page = query.page.unwrap_or(1).max(1);
    let board = board_info(&state, board_id).await?;
    let total_roots = i64::from(board.root_count);
    let total_pages = total_pages(total_roots, page_size);
    let page = if total_pages > 0 {
        page.min(total_pages)
    } else {
        1
    };
    let offset = (page - 1) * page_size;
    let posts = board_posts(&state, board_id, page_size, offset).await?;
    let trees = group_posts_by_tree(posts);
    let boards = board_navigation(&state).await?;

    Ok(Json(BoardResponse {
        site_name: state.site_name.clone(),
        board,
        pager: Pager {
            page,
            page_size,
            total_pages,
            total_roots,
            has_previous: page > 1,
            has_next: total_pages > 0 && page < total_pages,
        },
        trees,
        boards,
    }))
}

async fn board_info(state: &AppState, board_id: i32) -> AppResult<BoardInfo> {
    let board = sqlx::query_as::<_, BoardInfoRow>(
        r#"
        SELECT
            b.id,
            BTRIM(b.name) AS name,
            NULLIF(BTRIM(b.comment), '') AS comment,
            b.category_id,
            BTRIM(c.name) AS category_name,
            b.post_count,
            b.root_count,
            NULLIF(BTRIM(b.master_name), '') AS master_name,
            NULLIF(BTRIM(b.master_name_2), '') AS master_name_2,
            NULLIF(BTRIM(b.master_name_3), '') AS master_name_3,
            NULLIF(BTRIM(b.master_name_4), '') AS master_name_4
        FROM board b
        JOIN category c ON c.id = b.category_id
        WHERE b.id = $1
        "#,
    )
    .bind(board_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(BoardInfo {
        id: board.id,
        name: board.name,
        comment: board.comment,
        category_id: board.category_id,
        category_name: board.category_name,
        post_count: board.post_count,
        root_count: board.root_count.unwrap_or(0),
        master_names: [
            board.master_name,
            board.master_name_2,
            board.master_name_3,
            board.master_name_4,
        ]
        .into_iter()
        .flatten()
        .collect(),
    })
}

async fn board_posts(
    state: &AppState,
    board_id: i32,
    page_size: i64,
    offset: i64,
) -> AppResult<Vec<BoardPostSummary>> {
    let posts = sqlx::query_as::<_, BoardPostSummary>(
        r#"
        WITH roots AS (
            SELECT id, COALESCE(root_id, id) AS root_id
            FROM post
            WHERE board_id = $1
              AND COALESCE(parent_id, 0) = 0
              AND state <> 2
            ORDER BY COALESCE(root_id, id) DESC, id DESC
            LIMIT $2 OFFSET $3
        )
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
            p.reply_count,
            p.access_count,
            p.point,
            p.type AS post_type,
            p.state,
            NULLIF(BTRIM(p.image_url), '') AS image_url
        FROM roots r
        JOIN post p ON COALESCE(p.root_id, p.id) = r.id
        WHERE p.board_id = $1
          AND p.state <> 2
        ORDER BY r.root_id DESC, p.order_num
        "#,
    )
    .bind(board_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(posts)
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

fn total_pages(total_roots: i64, page_size: i64) -> i64 {
    if total_roots == 0 {
        0
    } else {
        (total_roots + page_size - 1) / page_size
    }
}

fn group_posts_by_tree(posts: Vec<BoardPostSummary>) -> Vec<PostTree> {
    let mut trees = Vec::<PostTree>::new();

    for post in posts {
        let root_id = post.root_id;
        match trees.last_mut() {
            Some(tree) if tree.root_id == root_id => tree.posts.push(post),
            _ => trees.push(PostTree {
                root_id,
                posts: vec![post],
            }),
        }
    }

    trees
}
