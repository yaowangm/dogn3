use crate::{
    auth::SessionStore,
    cache::RedisCache,
    rate_limit::{RateLimitConfig, RateLimiter},
};
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailDelivery {
    Sendmail,
    Smtp,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
    pub board_page_size: i64,
    pub post_reply_max_age_days: i32,
    pub post_reply_max_points: i32,
    pub new_user_initial_points: i32,
    pub root_post_regular_award_points: i32,
    pub root_post_forward_award_points: i32,
    pub root_post_original_award_points: i32,
    pub post_subject_max_length: usize,
    pub post_content_max_bytes: usize,
    pub post_signature_max_bytes: usize,
    pub image_directory: PathBuf,
    pub image_upload_max_bytes: usize,
    pub sessions: SessionStore,
    pub login_hash_permits: Arc<Semaphore>,
    pub password_reset: PasswordResetConfig,
    pub rate_limiter: RateLimiter,
}

#[derive(Clone, Copy)]
pub struct AuthRuntimeConfig {
    pub session_ttl: Duration,
    pub session_cookie_secure: bool,
    pub login_max_concurrent_hashes: usize,
}

#[derive(Clone)]
pub struct PasswordResetConfig {
    pub enabled: bool,
    pub mail_delivery: MailDelivery,
    pub sendmail_path: PathBuf,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub mail_from: Option<String>,
    pub public_site_url: Option<String>,
    pub ttl: Duration,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        cache: Option<RedisCache>,
        site_name: String,
        board_page_size: i64,
        post_reply_max_age_days: i32,
        post_reply_max_points: i32,
        new_user_initial_points: i32,
        root_post_regular_award_points: i32,
        root_post_forward_award_points: i32,
        root_post_original_award_points: i32,
        post_subject_max_length: usize,
        post_content_max_bytes: usize,
        post_signature_max_bytes: usize,
        image_directory: PathBuf,
        image_upload_max_bytes: usize,
        auth: AuthRuntimeConfig,
        password_reset: PasswordResetConfig,
        rate_limit: RateLimitConfig,
    ) -> Self {
        let rate_limiter = RateLimiter::new(rate_limit, cache.clone());
        Self {
            pool,
            cache: cache.clone(),
            site_name,
            board_page_size,
            post_reply_max_age_days,
            post_reply_max_points,
            new_user_initial_points,
            root_post_regular_award_points,
            root_post_forward_award_points,
            root_post_original_award_points,
            post_subject_max_length,
            post_content_max_bytes,
            post_signature_max_bytes,
            image_directory,
            image_upload_max_bytes,
            sessions: SessionStore::with_redis(
                auth.session_ttl,
                auth.session_cookie_secure,
                cache.clone(),
            ),
            login_hash_permits: Arc::new(Semaphore::new(auth.login_max_concurrent_hashes.max(1))),
            password_reset,
            rate_limiter,
        }
    }
}
