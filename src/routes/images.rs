use std::path::{Component, Path as FilePath, PathBuf};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
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

    let authenticated = auth::is_authenticated(&state, &headers).await?;
    if !image_access(&state, &path).await?.allows(authenticated) {
        return Ok(no_store_not_found());
    }

    let image_directory = tokio::fs::canonicalize(&state.image_directory)
        .await
        .map_err(|_| AppError::NotFound)?;
    let image_path = resolve_image_path(&image_directory, relative_path).await?;
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
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Clone, Copy, Debug)]
enum ImageAccess {
    Public,
    Authenticated,
    Denied,
}

impl ImageAccess {
    fn allows(self, authenticated: bool) -> bool {
        match self {
            Self::Public => true,
            Self::Authenticated => authenticated,
            Self::Denied => false,
        }
    }
}

async fn image_access(state: &AppState, relative_path: &str) -> AppResult<ImageAccess> {
    let (has_public_reference, has_encrypted_reference, has_any_reference) =
        sqlx::query_as::<_, (bool, bool, bool)>(
            r#"
        SELECT
            COALESCE(bool_or(state = 0), false),
            COALESCE(bool_or(state = 1), false),
            COUNT(*) > 0
        FROM post
        WHERE NULLIF(BTRIM(image_url), '') IS NOT NULL
          AND regexp_replace(regexp_replace(BTRIM(image_url), '^/+', ''), '^images/', '') = $1
        "#,
        )
        .bind(relative_path)
        .fetch_one(&state.pool)
        .await?;

    Ok(
        if has_public_reference || (!has_any_reference && !managed_upload_path(relative_path)) {
            ImageAccess::Public
        } else if has_encrypted_reference {
            ImageAccess::Authenticated
        } else {
            ImageAccess::Denied
        },
    )
}

fn managed_upload_path(relative_path: &str) -> bool {
    relative_path.starts_with("uploads/")
        || monthly_image_path(relative_path)
        || legacy_pic_monthly_image_path(relative_path).is_some()
}

fn monthly_image_path(relative_path: &str) -> bool {
    relative_path.split_once('/').is_some_and(|(month, file)| {
        month.len() == 6 && month.chars().all(|c| c.is_ascii_digit()) && !file.is_empty()
    })
}

fn legacy_pic_monthly_image_path(relative_path: &str) -> Option<&str> {
    relative_path
        .strip_prefix("pic/")
        .filter(|path| monthly_image_path(path))
}

async fn resolve_image_path(
    image_directory: &FilePath,
    relative_path: &FilePath,
) -> AppResult<PathBuf> {
    let Some(relative_path_str) = relative_path.to_str() else {
        return Err(AppError::NotFound);
    };

    if let Some(unprefixed_path) = legacy_pic_monthly_image_path(relative_path_str) {
        if let Ok(path) = tokio::fs::canonicalize(image_directory.join(unprefixed_path)).await {
            return Ok(path);
        }
    }

    tokio::fs::canonicalize(image_directory.join(relative_path))
        .await
        .map_err(|_| AppError::NotFound)
}

fn no_store_not_found() -> Response {
    let mut response = AppError::NotFound.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn image_content_type(path: &FilePath) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
