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
const POST_UPDATE_BODY_OVERHEAD_BYTES: usize = 1024 * 1024;

pub fn build_router(state: AppState) -> Router {
    let post_update_body_limit =
        post_update_body_limit(state.image_upload_max_bytes, state.post_content_max_bytes);

    Router::new()
        .merge(routes::page_router())
        .merge(routes::media_router())
        .nest("/api", routes::api_router(post_update_body_limit))
        .nest_service("/assets", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(versioned_asset_cache))
        .with_state(state)
}

fn post_update_body_limit(image_upload_max_bytes: usize, post_content_max_bytes: usize) -> usize {
    image_upload_max_bytes
        .saturating_mul(2)
        .saturating_add(post_content_max_bytes.saturating_mul(6))
        .saturating_add(POST_UPDATE_BODY_OVERHEAD_BYTES)
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

#[cfg(test)]
mod tests {
    use super::post_update_body_limit;

    #[test]
    fn post_update_body_limit_supports_hex_encoded_maximum_upload() {
        let image_bytes = 10 * 1024 * 1024;
        let content_bytes = 128 * 1024;

        let limit = post_update_body_limit(image_bytes, content_bytes);

        assert!(limit > image_bytes * 2 + content_bytes);
    }
}
