pub mod cache;
pub mod config;
pub mod error;
pub mod routes;
pub mod state;

use axum::Router;
use state::AppState;
use std::path::Path;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

pub fn build_router(state: AppState, image_directory: impl AsRef<Path>) -> Router {
    let image_directory = image_directory.as_ref();

    Router::new()
        .merge(routes::page_router())
        .nest("/api", routes::api_router())
        .nest_service("/assets", ServeDir::new("static"))
        .nest_service("/images", ServeDir::new(image_directory))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
