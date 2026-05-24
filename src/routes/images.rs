use std::path::{Component, Path as FilePath};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::{
    error::{AppError, AppResult},
    routes::auth,
    state::AppState,
};

pub async fn image(
    Path(path): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let relative_path = FilePath::new(&path);
    if path.contains('\\')
        || !relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(AppError::NotFound);
    }

    let Some(content_type) = image_content_type(relative_path) else {
        return Err(AppError::NotFound);
    };

    if image_is_encrypted_only(&state, &path).await? && !auth::is_authenticated(&state, &headers) {
        return Err(AppError::NotFound);
    }

    let image_directory = tokio::fs::canonicalize(&state.image_directory)
        .await
        .map_err(|_| AppError::NotFound)?;
    let image_path = tokio::fs::canonicalize(image_directory.join(relative_path))
        .await
        .map_err(|_| AppError::NotFound)?;
    if !image_path.starts_with(&image_directory) {
        return Err(AppError::NotFound);
    }

    let bytes = tokio::fs::read(image_path)
        .await
        .map_err(|_| AppError::NotFound)?;

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

async fn image_is_encrypted_only(state: &AppState, relative_path: &str) -> AppResult<bool> {
    let restricted = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT
            EXISTS (
                SELECT 1
                FROM post
                WHERE state = 1
                  AND regexp_replace(regexp_replace(BTRIM(image_url), '^/+', ''), '^images/', '') = $1
            )
            AND NOT EXISTS (
                SELECT 1
                FROM post
                WHERE state = 0
                  AND regexp_replace(regexp_replace(BTRIM(image_url), '^/+', ''), '^images/', '') = $1
            )
        "#,
    )
    .bind(relative_path)
    .fetch_one(&state.pool)
    .await?;

    Ok(restricted)
}

fn image_content_type(path: &FilePath) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
