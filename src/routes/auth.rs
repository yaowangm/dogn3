use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    auth::{
        AuthenticatedUser, MIGRATED_PASSWORD_SCHEME, MODERN_PASSWORD_SCHEME, hash_migrated_input,
        hash_modern_password, legacy_password_input, verify_migrated_password,
        verify_modern_password,
    },
    error::AppResult,
    state::AppState,
};

const SESSION_COOKIE_NAME: &str = "dogn_session";
const SAME_ORIGIN_REQUEST_HEADER: &str = "x-dogn-request";
const ADMIN_LEVEL: i32 = 10;
const MIN_PASSWORD_LENGTH: usize = 8;
const MAX_PASSWORD_LENGTH: usize = 30;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    current_password: Option<String>,
    new_password: String,
    confirm_password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    site_name: String,
    authenticated: bool,
    user: Option<AuthenticatedUser>,
}

#[derive(Debug, Serialize)]
struct LoginErrorResponse {
    error: LoginError,
}

#[derive(Debug, Serialize)]
struct LoginError {
    code: &'static str,
    message: &'static str,
}

#[derive(Debug, Serialize)]
struct PasswordChangeResponse {
    changed: bool,
    target_user_id: i32,
    session_invalidated: bool,
}

#[derive(Debug, FromRow)]
struct Credential {
    id: i32,
    name: String,
    level: i32,
    password: String,
    password_scheme: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> AppResult<Response> {
    let Ok(_permit) = state.login_hash_permits.clone().try_acquire_owned() else {
        return Ok(login_busy());
    };

    let name = request.name.trim();
    let credential = if name.is_empty() || request.password.is_empty() {
        None
    } else {
        sqlx::query_as::<_, Credential>(
            r#"
            SELECT id, BTRIM(name) AS name, level, password, password_scheme
            FROM user_info
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&state.pool)
        .await?
    };

    let authenticated = credential.as_ref().is_some_and(|credential| {
        credential.level != 0 && verify_password(credential, &request.password)
    });

    if !authenticated {
        // Pay the password-hash cost for credentials that are absent or not eligible.
        if credential.as_ref().is_none_or(|credential| {
            credential.level == 0 || !is_supported_scheme(credential.password_scheme.as_deref())
        }) {
            let _ = hash_migrated_input(&legacy_password_input(&request.password));
        }
        return Ok(auth_failure());
    }

    let credential = credential.expect("authenticated credential must exist");
    let user = AuthenticatedUser {
        id: credential.id,
        name: credential.name,
        level: credential.level,
    };
    let token = state.sessions.create(user.clone());

    Ok((
        [
            (header::SET_COOKIE, session_cookie(&state, &token)),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        Json(SessionResponse {
            site_name: state.site_name.clone(),
            authenticated: true,
            user: Some(user),
        }),
    )
        .into_response())
}

pub async fn change_password(
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> AppResult<Response> {
    if headers
        .get(SAME_ORIGIN_REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
        != Some("fetch")
    {
        return Ok(password_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = current_user(&state, &headers) else {
        return Ok(password_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Login is required to change a password.",
        ));
    };
    let is_admin = viewer.level >= ADMIN_LEVEL;
    if viewer.id != user_id && !is_admin {
        return Ok(password_error(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "You are not authorized to change this password.",
        ));
    }
    if request.new_password != request.confirm_password {
        return Ok(password_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "password_confirmation_mismatch",
            "The new password confirmation does not match.",
        ));
    }
    if let Err(message) = validate_new_password(&request.new_password) {
        return Ok(password_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_new_password",
            message,
        ));
    }

    let Ok(_permit) = state.login_hash_permits.clone().try_acquire_owned() else {
        return Ok(login_busy());
    };
    let target = sqlx::query_as::<_, Credential>(
        r#"
        SELECT id, BTRIM(name) AS name, level, password, password_scheme
        FROM user_info
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(target) = target else {
        return Ok(password_error(
            StatusCode::NOT_FOUND,
            "user_not_found",
            "The requested user was not found.",
        ));
    };

    if !is_admin {
        let current_password = request.current_password.as_deref().unwrap_or("");
        if !verify_password(&target, current_password) {
            return Ok(password_error(
                StatusCode::UNAUTHORIZED,
                "invalid_current_password",
                "The current password is incorrect.",
            ));
        }
    }

    let password = hash_modern_password(&request.new_password)?;
    let update = if is_admin {
        sqlx::query(
            r#"
            UPDATE user_info
            SET password = $1, password_scheme = $2
            WHERE id = $3
            "#,
        )
        .bind(password)
        .bind(MODERN_PASSWORD_SCHEME)
        .bind(user_id)
        .execute(&state.pool)
        .await?
    } else {
        sqlx::query(
            r#"
            UPDATE user_info
            SET password = $1, password_scheme = $2
            WHERE id = $3
              AND password = $4
              AND password_scheme IS NOT DISTINCT FROM $5
            "#,
        )
        .bind(password)
        .bind(MODERN_PASSWORD_SCHEME)
        .bind(user_id)
        .bind(&target.password)
        .bind(&target.password_scheme)
        .execute(&state.pool)
        .await?
    };
    if update.rows_affected() != 1 {
        return Ok(password_error(
            StatusCode::CONFLICT,
            "credential_changed",
            "The password changed during this request. Try again.",
        ));
    }
    state.sessions.remove_user(user_id);

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(PasswordChangeResponse {
            changed: true,
            target_user_id: user_id,
            session_invalidated: viewer.id == user_id,
        }),
    )
        .into_response())
}

pub async fn session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = session_token(&headers).and_then(|token| state.sessions.get(token));
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(SessionResponse {
            site_name: state.site_name.clone(),
            authenticated: user.is_some(),
            user,
        }),
    )
        .into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        state.sessions.remove(token);
    }

    (
        [
            (header::SET_COOKIE, cleared_session_cookie(&state)),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        Json(SessionResponse {
            site_name: state.site_name.clone(),
            authenticated: false,
            user: None,
        }),
    )
        .into_response()
}

pub(super) fn is_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    current_user(state, headers).is_some()
}

pub(super) fn current_user(state: &AppState, headers: &HeaderMap) -> Option<AuthenticatedUser> {
    session_token(headers).and_then(|token| state.sessions.get(token))
}

fn auth_failure() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(LoginErrorResponse {
            error: LoginError {
                code: "invalid_credentials",
                message: "Invalid user name or password.",
            },
        }),
    )
        .into_response()
}

fn verify_password(credential: &Credential, raw_password: &str) -> bool {
    match credential.password_scheme.as_deref() {
        Some(MIGRATED_PASSWORD_SCHEME) => {
            verify_migrated_password(raw_password, &credential.password)
        }
        Some(MODERN_PASSWORD_SCHEME) => verify_modern_password(raw_password, &credential.password),
        _ => false,
    }
}

fn is_supported_scheme(scheme: Option<&str>) -> bool {
    matches!(
        scheme,
        Some(MIGRATED_PASSWORD_SCHEME) | Some(MODERN_PASSWORD_SCHEME)
    )
}

fn validate_new_password(password: &str) -> Result<(), &'static str> {
    let length = password.chars().count();
    if !(MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&length) {
        return Err("Password must be 8 to 30 characters long.");
    }
    if !password
        .chars()
        .all(|character| character.is_ascii_graphic())
    {
        return Err("Password may contain printable ASCII characters only, without spaces.");
    }
    if !password
        .chars()
        .any(|character| character.is_ascii_alphabetic())
        || !password.chars().any(|character| character.is_ascii_digit())
        || !password
            .chars()
            .any(|character| character.is_ascii_punctuation())
    {
        return Err("Password must contain a letter, a number, and a symbol.");
    }
    Ok(())
}

fn password_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(LoginErrorResponse {
            error: LoginError { code, message },
        }),
    )
        .into_response()
}

fn login_busy() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::RETRY_AFTER, "1"),
        ],
        Json(LoginErrorResponse {
            error: LoginError {
                code: "login_busy",
                message: "Too many login requests. Try again shortly.",
            },
        }),
    )
        .into_response()
}

fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| {
            let (name, value) = cookie.split_once('=')?;
            (name == SESSION_COOKIE_NAME).then_some(value)
        })
        .filter(|token| !token.is_empty())
}

fn session_cookie(state: &AppState, token: &str) -> String {
    format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        state.sessions.max_age_seconds(),
        secure_cookie_suffix(state)
    )
}

fn cleared_session_cookie(state: &AppState) -> String {
    format!(
        "{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_cookie_suffix(state)
    )
}

fn secure_cookie_suffix(state: &AppState) -> &'static str {
    if state.sessions.cookie_secure() {
        "; Secure"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::validate_new_password;

    #[test]
    fn password_policy_accepts_requested_ascii_mix() {
        assert!(validate_new_password("Forum123!").is_ok());
    }

    #[test]
    fn password_policy_rejects_missing_class_or_non_ascii_input() {
        assert!(validate_new_password("ForumPassword!").is_err());
        assert!(validate_new_password("Forum123").is_err());
        assert!(validate_new_password("论坛Forum123!").is_err());
        assert!(validate_new_password("Forum 123!").is_err());
    }
}
