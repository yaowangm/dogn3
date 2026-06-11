use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::AppResult, state::AppState};

const NAVIGATION_CACHE_KEY: &str = "api:navigation:v1";

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct BoardNavSummary {
    pub id: i32,
    pub name: String,
    pub category_id: i32,
    pub category_name: String,
}

pub async fn boards(state: &AppState) -> AppResult<Vec<BoardNavSummary>> {
    if let Some(cache) = &state.cache
        && cache.is_enabled()
    {
        match cache
            .get_json::<Vec<BoardNavSummary>>(NAVIGATION_CACHE_KEY)
            .await
        {
            Ok(Some(boards)) => return Ok(boards),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = ?error, "failed to read board navigation cache");
            }
        }
    }

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

    if let Some(cache) = &state.cache
        && cache.is_enabled()
        && let Err(error) = cache.set_json(NAVIGATION_CACHE_KEY, &boards).await
    {
        tracing::warn!(error = ?error, "failed to write board navigation cache");
    }

    Ok(boards)
}

pub async fn invalidate_cache(state: &AppState) {
    let Some(cache) = &state.cache else {
        return;
    };
    if !cache.is_enabled() {
        return;
    }
    if let Err(error) = cache.delete(NAVIGATION_CACHE_KEY).await {
        tracing::warn!(error = ?error, "failed to invalidate board navigation cache");
    }
}
