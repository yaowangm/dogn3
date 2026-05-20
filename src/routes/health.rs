use axum::{Json, extract::State};
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(HealthResponse {
        status: "ok",
        database: "ok",
    }))
}
