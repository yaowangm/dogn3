use axum::{
    Extension, Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::{
    io::Write,
    net::SocketAddr,
    process::{Command, Stdio},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

use crate::{
    auth::{
        AuthenticatedUser, MIGRATED_PASSWORD_SCHEME, MODERN_PASSWORD_SCHEME, hash_migrated_input,
        hash_modern_password, legacy_password_input, verify_migrated_password,
        verify_modern_password,
    },
    error::AppResult,
    rate_limit::RateLimitError,
    state::{AppState, MailDelivery},
};

const SESSION_COOKIE_NAME: &str = "dogn_session";
const SAME_ORIGIN_REQUEST_HEADER: &str = "x-dogn-request";
const ADMIN_LEVEL: i32 = 10;
const MIN_PASSWORD_LENGTH: usize = 8;
const MAX_PASSWORD_LENGTH: usize = 30;
const PASSWORD_RESET_GENERIC_MESSAGE: &str =
    "If the email exists, a password reset message has been sent.";

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

#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirmRequest {
    token: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    site_name: String,
    authenticated: bool,
    user: Option<AuthenticatedUser>,
    expires_at_epoch_ms: Option<u64>,
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

#[derive(Debug, Serialize)]
struct PasswordResetRequestResponse {
    requested: bool,
    message: &'static str,
}

#[derive(Debug, Serialize)]
struct PasswordResetConfirmResponse {
    changed: bool,
}

#[derive(Debug, FromRow)]
struct Credential {
    id: i32,
    name: String,
    level: i32,
    password: String,
    password_scheme: Option<String>,
}

#[derive(Debug, FromRow)]
struct ResetAccount {
    id: i32,
    name: String,
    email: String,
}

#[derive(Debug, FromRow)]
struct ResetTokenRow {
    id: i32,
    user_id: i32,
}

pub async fn login(
    State(state): State<AppState>,
    connection: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(request): Json<LoginRequest>,
) -> AppResult<Response> {
    let client_ip = connection
        .as_ref()
        .map(|Extension(ConnectInfo(address))| address.ip().to_string());
    let name = request.name.trim();
    match rate_limit_is_blocked(
        state
            .rate_limiter
            .login_is_blocked(name, client_ip.as_deref())
            .await,
    ) {
        Ok(true) => {
            tracing::warn!(bucket = "login", "rate limit blocked login attempt");
            return Ok(rate_limit_failure());
        }
        Ok(false) => {}
        Err(response) => return Ok(response),
    }

    let Ok(_permit) = state.login_hash_permits.clone().try_acquire_owned() else {
        return Ok(login_busy());
    };

    let credential = if name.is_empty() || request.password.is_empty() {
        None
    } else {
        sqlx::query_as::<_, Credential>(
            r#"
            SELECT id, BTRIM(name) AS name, level, password, password_scheme
            FROM user_info
            WHERE BTRIM(name) = $1
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
        if let Some(credential) = &credential {
            sqlx::query(
                r#"
                UPDATE user_info
                SET log_error_time = CURRENT_TIMESTAMP,
                    log_error_count = COALESCE(log_error_count, 0) + 1
                WHERE id = $1
                "#,
            )
            .bind(credential.id)
            .execute(&state.pool)
            .await?;
        }
        match rate_limit_is_blocked(
            state
                .rate_limiter
                .record_login_failure(name, client_ip.as_deref())
                .await,
        ) {
            Ok(true) => {
                tracing::warn!(bucket = "login", "rate limit locked login attempts");
                return Ok(rate_limit_failure());
            }
            Ok(false) => {}
            Err(response) => return Ok(response),
        }
        return Ok(match credential.as_ref() {
            Some(credential) if credential.level == 0 => frozen_account_failure(),
            _ => auth_failure(),
        });
    }

    let credential = credential.expect("authenticated credential must exist");
    if let Err(error) = state.rate_limiter.clear_login_user(name).await {
        tracing::warn!(?error, "failed to clear login rate limit bucket");
    }
    sqlx::query(
        r#"
        UPDATE user_info
        SET last_login = CURRENT_TIMESTAMP,
            last_login_ip = COALESCE($1, last_login_ip),
            login_count = COALESCE(login_count, 0) + 1
        WHERE id = $2
        "#,
    )
    .bind(client_ip)
    .bind(credential.id)
    .execute(&state.pool)
    .await?;
    let user = AuthenticatedUser {
        id: credential.id,
        name: credential.name,
        level: credential.level,
    };
    let token = state.sessions.create_persistent(user.clone()).await;

    Ok((
        [
            (header::SET_COOKIE, session_cookie(&state, &token)),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        Json(SessionResponse {
            site_name: state.site_name.clone(),
            authenticated: true,
            user: Some(user),
            expires_at_epoch_ms: state.sessions.persistent_expires_at_epoch_ms(&token).await,
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
    if !mutation_request_is_verified(&headers) {
        return Ok(password_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    let Some(viewer) = current_user(&state, &headers).await? else {
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
    state.sessions.remove_user_persistent(user_id).await;

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

pub async fn request_password_reset(
    State(state): State<AppState>,
    connection: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<PasswordResetRequest>,
) -> AppResult<Response> {
    if !mutation_request_is_verified(&headers) {
        return Ok(password_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    if !state.password_reset.enabled {
        return Ok(password_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_disabled",
            "Password reset is not available.",
        ));
    }

    let email = request.email.trim();
    let rate_limit_email = if header_value_is_unsafe(email) {
        ""
    } else {
        email
    };
    let client_ip = connection
        .as_ref()
        .map(|Extension(ConnectInfo(address))| address.ip().to_string());
    match rate_limit_is_blocked(
        state
            .rate_limiter
            .password_reset_request_is_blocked(rate_limit_email, client_ip.as_deref())
            .await,
    ) {
        Ok(true) => {
            tracing::warn!(
                bucket = "reset_request",
                "rate limit blocked password reset request"
            );
            return Ok(password_reset_request_ack());
        }
        Ok(false) => {}
        Err(response) => return Ok(response),
    }
    if email.is_empty() || header_value_is_unsafe(email) {
        return Ok(password_reset_request_ack());
    }

    let accounts = sqlx::query_as::<_, ResetAccount>(
        r#"
        SELECT id, BTRIM(name) AS name, BTRIM(email) AS email
        FROM user_info
        WHERE LOWER(BTRIM(email)) = LOWER($1)
          AND level <> 0
        LIMIT 2
        "#,
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await?;
    if accounts.len() != 1 {
        return Ok(password_reset_request_ack());
    }

    let account = accounts.into_iter().next().expect("one account");
    if header_value_is_unsafe(&account.email) {
        tracing::warn!(
            user_id = account.id,
            "refusing password reset email with unsafe address"
        );
        return Ok(password_reset_request_ack());
    }
    let Some(mail_from) = state.password_reset.mail_from.clone() else {
        return Ok(password_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_reset_misconfigured",
            "Password reset mail is not configured.",
        ));
    };
    let Some(public_site_url) = state.password_reset.public_site_url.clone() else {
        return Ok(password_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_reset_misconfigured",
            "Password reset mail is not configured.",
        ));
    };
    if header_value_is_unsafe(&mail_from) || header_value_is_unsafe(&public_site_url) {
        return Ok(password_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_reset_misconfigured",
            "Password reset mail is not configured.",
        ));
    }

    let raw_token = reset_token();
    let token_hash = reset_token_hash(&raw_token);
    let reset_url = format!("{public_site_url}/reset_password?token={raw_token}");
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE password_reset_token
        SET used_at = CURRENT_TIMESTAMP
        WHERE user_id = $1
          AND used_at IS NULL
        "#,
    )
    .bind(account.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO password_reset_token (user_id, token_hash, expires_at)
        VALUES ($1, $2, CURRENT_TIMESTAMP + ($3 * INTERVAL '1 second'))
        "#,
    )
    .bind(account.id)
    .bind(&token_hash)
    .bind(i64::try_from(state.password_reset.ttl.as_secs()).unwrap_or(i64::MAX))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let message = reset_email(&mail_from, &account.email, &account.name, &reset_url);
    if let Err(error) = send_mail(&state, &mail_from, &account.email, message).await {
        tracing::error!(
            ?error,
            user_id = account.id,
            "failed to send password reset email"
        );
        sqlx::query(
            r#"
            UPDATE password_reset_token
            SET used_at = CURRENT_TIMESTAMP
            WHERE user_id = $1
              AND token_hash = $2
              AND used_at IS NULL
            "#,
        )
        .bind(account.id)
        .bind(&token_hash)
        .execute(&state.pool)
        .await?;
        return Ok(password_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_reset_mail_failed",
            "Password reset mail could not be sent.",
        ));
    }

    Ok(password_reset_request_ack())
}

pub async fn confirm_password_reset(
    State(state): State<AppState>,
    connection: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<PasswordResetConfirmRequest>,
) -> AppResult<Response> {
    if !mutation_request_is_verified(&headers) {
        return Ok(password_error(
            StatusCode::FORBIDDEN,
            "csrf_check_failed",
            "This request could not be verified.",
        ));
    }
    if !state.password_reset.enabled {
        return Ok(password_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_disabled",
            "Password reset is not available.",
        ));
    }
    let client_ip = connection
        .as_ref()
        .map(|Extension(ConnectInfo(address))| address.ip().to_string());
    match rate_limit_is_blocked(
        state
            .rate_limiter
            .password_reset_confirm_is_blocked(client_ip.as_deref())
            .await,
    ) {
        Ok(true) => {
            tracing::warn!(
                bucket = "reset_confirm",
                "rate limit blocked password reset confirmation"
            );
            return Ok(rate_limit_failure());
        }
        Ok(false) => {}
        Err(response) => return Ok(response),
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
    let token = request.token.trim();
    if token.is_empty() || !token.chars().all(|character| character.is_ascii_hexdigit()) {
        match invalid_reset_confirm_is_now_blocked(&state, client_ip.as_deref()).await {
            Ok(true) => return Ok(rate_limit_failure()),
            Ok(false) => {}
            Err(response) => return Ok(response),
        }
        return Ok(invalid_reset_token());
    }
    let Ok(_permit) = state.login_hash_permits.clone().try_acquire_owned() else {
        return Ok(login_busy());
    };
    let password = hash_modern_password(&request.new_password)?;
    let token_hash = reset_token_hash(token);
    let mut transaction = state.pool.begin().await?;
    let token_row = sqlx::query_as::<_, ResetTokenRow>(
        r#"
        SELECT id, user_id
        FROM password_reset_token
        WHERE token_hash = $1
          AND used_at IS NULL
          AND expires_at > CURRENT_TIMESTAMP
        FOR UPDATE
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(token_row) = token_row else {
        transaction.rollback().await?;
        match invalid_reset_confirm_is_now_blocked(&state, client_ip.as_deref()).await {
            Ok(true) => return Ok(rate_limit_failure()),
            Ok(false) => {}
            Err(response) => return Ok(response),
        }
        return Ok(invalid_reset_token());
    };
    let updated = sqlx::query(
        r#"
        UPDATE user_info
        SET password = $1,
            password_scheme = $2
        WHERE id = $3
          AND level <> 0
        "#,
    )
    .bind(password)
    .bind(MODERN_PASSWORD_SCHEME)
    .bind(token_row.user_id)
    .execute(&mut *transaction)
    .await?;
    if updated.rows_affected() != 1 {
        transaction.rollback().await?;
        match invalid_reset_confirm_is_now_blocked(&state, client_ip.as_deref()).await {
            Ok(true) => return Ok(rate_limit_failure()),
            Ok(false) => {}
            Err(response) => return Ok(response),
        }
        return Ok(invalid_reset_token());
    }
    sqlx::query("UPDATE password_reset_token SET used_at = CURRENT_TIMESTAMP WHERE id = $1")
        .bind(token_row.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    state
        .sessions
        .remove_user_persistent(token_row.user_id)
        .await;

    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(PasswordResetConfirmResponse { changed: true }),
    )
        .into_response())
}

pub async fn session(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let current = current_session(&state, &headers).await?;
    let (token, user) = match current {
        Some((token, user)) => (Some(token), Some(user)),
        None => (None, None),
    };
    let expires_at_epoch_ms = match token.as_deref() {
        Some(token) => state.sessions.persistent_expires_at_epoch_ms(token).await,
        None => None,
    };
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(SessionResponse {
            site_name: state.site_name.clone(),
            authenticated: user.is_some(),
            user,
            expires_at_epoch_ms,
        }),
    )
        .into_response())
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        state.sessions.remove_persistent(token).await;
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
            expires_at_epoch_ms: None,
        }),
    )
        .into_response()
}

pub(super) async fn is_authenticated(state: &AppState, headers: &HeaderMap) -> AppResult<bool> {
    Ok(current_user(state, headers).await?.is_some())
}

pub(super) async fn current_user(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<Option<AuthenticatedUser>> {
    Ok(current_session(state, headers).await?.map(|(_, user)| user))
}

pub(super) async fn current_session(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<Option<(String, AuthenticatedUser)>> {
    let Some(token) = session_token(headers) else {
        return Ok(None);
    };
    let Some(session_user) = state.sessions.get_persistent(token).await else {
        return Ok(None);
    };
    let current_user = sqlx::query_as::<_, (i32, String, i32)>(
        r#"
        SELECT id, BTRIM(name), level
        FROM user_info
        WHERE id = $1
          AND level <> 0
        "#,
    )
    .bind(session_user.id)
    .fetch_optional(&state.pool)
    .await?
    .map(|(id, name, level)| AuthenticatedUser { id, name, level });
    if current_user.is_none() {
        state.sessions.remove_persistent(token).await;
    }
    Ok(current_user.map(|user| (token.to_string(), user)))
}

pub(super) fn mutation_request_is_verified(headers: &HeaderMap) -> bool {
    headers
        .get(SAME_ORIGIN_REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
        == Some("fetch")
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

fn frozen_account_failure() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(LoginErrorResponse {
            error: LoginError {
                code: "account_frozen",
                message: "This account is frozen. Contact an administrator.",
            },
        }),
    )
        .into_response()
}

fn verify_password(credential: &Credential, raw_password: &str) -> bool {
    match credential.password_scheme.as_deref().map(str::trim) {
        Some(MIGRATED_PASSWORD_SCHEME) => {
            verify_migrated_password(raw_password, &credential.password)
        }
        Some(MODERN_PASSWORD_SCHEME) => verify_modern_password(raw_password, &credential.password),
        _ => false,
    }
}

fn is_supported_scheme(scheme: Option<&str>) -> bool {
    matches!(
        scheme.map(str::trim),
        Some(MIGRATED_PASSWORD_SCHEME) | Some(MODERN_PASSWORD_SCHEME)
    )
}

pub(super) fn validate_new_password(password: &str) -> Result<(), &'static str> {
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

fn rate_limit_is_blocked(result: Result<bool, RateLimitError>) -> Result<bool, Response> {
    match result {
        Ok(blocked) => Ok(blocked),
        Err(error) => {
            tracing::error!(?error, "rate limit backend failed");
            Err(password_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "rate_limit_unavailable",
                "Authentication rate limiting is unavailable.",
            ))
        }
    }
}

fn rate_limit_failure() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::RETRY_AFTER, "60"),
        ],
        Json(LoginErrorResponse {
            error: LoginError {
                code: "too_many_attempts",
                message: "Too many attempts. Try again later.",
            },
        }),
    )
        .into_response()
}

async fn invalid_reset_confirm_is_now_blocked(
    state: &AppState,
    ip: Option<&str>,
) -> Result<bool, Response> {
    rate_limit_is_blocked(
        state
            .rate_limiter
            .record_invalid_password_reset_confirm(ip)
            .await,
    )
}

fn password_reset_request_ack() -> Response {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(PasswordResetRequestResponse {
            requested: true,
            message: PASSWORD_RESET_GENERIC_MESSAGE,
        }),
    )
        .into_response()
}

fn invalid_reset_token() -> Response {
    password_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_reset_token",
        "The password reset link is invalid or expired.",
    )
}

fn reset_token() -> String {
    use argon2::password_hash::rand_core::RngCore;

    let mut bytes = [0_u8; 32];
    argon2::password_hash::rand_core::OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}

fn reset_token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn reset_email(from: &str, to: &str, name: &str, reset_url: &str) -> String {
    format!(
        "From: {from}\nTo: {to}\nSubject: Reset your Dogn password\nContent-Type: text/plain; charset=UTF-8\n\nHello {name},\n\nUse this link to reset your password:\n\n{reset_url}\n\nIf you did not request this, ignore this message.\n"
    )
}

fn header_value_is_unsafe(value: &str) -> bool {
    value.contains(|character| matches!(character, '\r' | '\n'))
}

async fn send_mail(state: &AppState, from: &str, to: &str, message: String) -> anyhow::Result<()> {
    match state.password_reset.mail_delivery {
        MailDelivery::Sendmail => send_mail_with_sendmail(state, message).await,
        MailDelivery::Smtp => send_mail_with_smtp(state, from, to, &message).await,
    }
}

async fn send_mail_with_sendmail(state: &AppState, message: String) -> anyhow::Result<()> {
    let sendmail_path = state.password_reset.sendmail_path.clone();
    tokio::task::spawn_blocking(move || {
        let mut child = Command::new(sendmail_path)
            .arg("-t")
            .arg("-oi")
            .stdin(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .as_mut()
            .expect("sendmail stdin should be piped")
            .write_all(message.as_bytes())?;
        let status = child.wait()?;
        anyhow::ensure!(status.success(), "sendmail exited with status {status}");
        Ok::<(), anyhow::Error>(())
    })
    .await?
}

async fn send_mail_with_smtp(
    state: &AppState,
    from: &str,
    to: &str,
    message: &str,
) -> anyhow::Result<()> {
    let address = format!(
        "{}:{}",
        state.password_reset.smtp_host, state.password_reset.smtp_port
    );
    let stream = TcpStream::connect(&address).await?;
    let mut smtp = BufReader::new(stream);

    read_smtp_response(&mut smtp, 220).await?;
    smtp_command(&mut smtp, "EHLO localhost\r\n", 250).await?;
    smtp_command(&mut smtp, &format!("MAIL FROM:<{from}>\r\n"), 250).await?;
    smtp_command(&mut smtp, &format!("RCPT TO:<{to}>\r\n"), 250).await?;
    smtp_command(&mut smtp, "DATA\r\n", 354).await?;
    smtp.get_mut()
        .write_all(smtp_data(message).as_bytes())
        .await?;
    read_smtp_response(&mut smtp, 250).await?;
    smtp_command(&mut smtp, "QUIT\r\n", 221).await?;

    Ok(())
}

async fn smtp_command(
    smtp: &mut BufReader<TcpStream>,
    command: &str,
    expected_code: u16,
) -> anyhow::Result<()> {
    smtp.get_mut().write_all(command.as_bytes()).await?;
    read_smtp_response(smtp, expected_code).await
}

async fn read_smtp_response(
    smtp: &mut BufReader<TcpStream>,
    expected_code: u16,
) -> anyhow::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = smtp.read_line(&mut line).await?;
        anyhow::ensure!(bytes > 0, "SMTP server closed the connection");
        let code = line
            .get(0..3)
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        anyhow::ensure!(
            code == expected_code,
            "SMTP expected {expected_code}, received {}",
            line.trim_end()
        );
        if line
            .as_bytes()
            .get(3)
            .is_none_or(|separator| *separator == b' ')
        {
            return Ok(());
        }
    }
}

fn smtp_data(message: &str) -> String {
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::new();
    for line in normalized.split('\n') {
        if line.starts_with('.') {
            output.push('.');
        }
        output.push_str(line);
        output.push_str("\r\n");
    }
    output.push_str(".\r\n");
    output
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
    use super::{reset_token, reset_token_hash, validate_new_password};

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

    #[test]
    fn reset_tokens_are_random_and_hashed() {
        let first = reset_token();
        let second = reset_token();

        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
        assert_eq!(reset_token_hash(&first).len(), 64);
        assert_ne!(reset_token_hash(&first), first);
    }
}
