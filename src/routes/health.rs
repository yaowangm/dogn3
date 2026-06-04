use axum::{Json, extract::State};
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    database: &'static str,
    cache: &'static str,
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    if let Err(error) = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        tracing::error!(error = %error, "health check failed while querying PostgreSQL");
        return Err(error.into());
    }

    let cache = match &state.cache {
        Some(cache) => {
            if let Err(error) = cache.ping().await {
                tracing::error!(error = %error, "health check failed while pinging Redis");
                return Err(error.into());
            }
            "ok"
        }
        None => "disabled",
    };

    Ok(Json(HealthResponse {
        status: "ok",
        database: "ok",
        cache,
    }))
}
