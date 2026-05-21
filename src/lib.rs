pub mod config;
pub mod error;
pub mod routes;
pub mod state;

use axum::Router;
use state::AppState;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(routes::page_router())
        .nest("/api", routes::api_router())
        .nest_service("/assets", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
