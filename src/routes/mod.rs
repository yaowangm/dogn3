mod board;
mod health;
mod home;
mod pages;
mod post;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/boards/{board_id}", get(board::board))
        .route("/posts/{post_id}", get(post::post))
        .route("/health", get(health::health))
        .route("/home", get(home::home))
}

pub fn page_router() -> Router<AppState> {
    Router::new()
        .route("/", get(pages::index))
        .route("/board/{board_id}", get(pages::index))
        .route("/post/{post_id}", get(pages::index))
}
