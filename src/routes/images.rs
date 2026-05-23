use std::path::{Component, Path as FilePath};

use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

pub async fn image(Path(path): Path<String>, State(state): State<AppState>) -> AppResult<Response> {
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
    let bytes = tokio::fs::read(state.image_directory.join(relative_path))
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

fn image_content_type(path: &FilePath) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
