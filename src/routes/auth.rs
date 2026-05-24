use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    auth::{
        AuthenticatedUser, MIGRATED_PASSWORD_SCHEME, hash_migrated_input, legacy_password_input,
        verify_migrated_password,
    },
    error::AppResult,
    state::AppState,
};

const SESSION_COOKIE_NAME: &str = "dogn_session";

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    name: String,
    password: String,
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
        credential.level != 0
            && credential.password_scheme.as_deref() == Some(MIGRATED_PASSWORD_SCHEME)
            && verify_migrated_password(&request.password, &credential.password)
    });

    if !authenticated {
        // Pay the password-hash cost for credentials that are absent or not eligible.
        if credential.as_ref().is_none_or(|credential| {
            credential.level == 0
                || credential.password_scheme.as_deref() != Some(MIGRATED_PASSWORD_SCHEME)
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
    session_token(headers)
        .and_then(|token| state.sessions.get(token))
        .is_some()
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
