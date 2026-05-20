mod health;
mod home;
mod pages;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/home", get(home::home))
}

pub fn page_router() -> Router<AppState> {
    Router::new().route("/", get(pages::index))
}
