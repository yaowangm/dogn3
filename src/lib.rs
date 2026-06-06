pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod rate_limit;
pub mod routes;
pub mod state;

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
};
use state::AppState;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

const VERSIONED_VENDOR_ASSET_PREFIX: &str = "/assets/vendor/katex-0.16.22/";

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(routes::page_router())
        .merge(routes::media_router())
        .nest("/api", routes::api_router())
        .nest_service("/assets", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(versioned_asset_cache))
        .with_state(state)
}

async fn versioned_asset_cache(request: Request, next: Next) -> Response {
    let immutable = request
        .uri()
        .path()
        .starts_with(VERSIONED_VENDOR_ASSET_PREFIX);
    let mut response = next.run(request).await;
    if immutable && response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}
