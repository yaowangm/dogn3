use crate::{auth::SessionStore, cache::RedisCache};
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
    pub board_page_size: i64,
    pub post_reply_max_age_days: i32,
    pub post_reply_max_points: i32,
    pub post_subject_max_length: usize,
    pub post_content_max_bytes: usize,
    pub image_directory: PathBuf,
    pub image_upload_max_bytes: usize,
    pub sessions: SessionStore,
    pub login_hash_permits: Arc<Semaphore>,
}

#[derive(Clone, Copy)]
pub struct AuthRuntimeConfig {
    pub session_ttl: Duration,
    pub session_cookie_secure: bool,
    pub login_max_concurrent_hashes: usize,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        cache: Option<RedisCache>,
        site_name: String,
        board_page_size: i64,
        post_reply_max_age_days: i32,
        post_reply_max_points: i32,
        post_subject_max_length: usize,
        post_content_max_bytes: usize,
        image_directory: PathBuf,
        image_upload_max_bytes: usize,
        auth: AuthRuntimeConfig,
    ) -> Self {
        Self {
            pool,
            cache,
            site_name,
            board_page_size,
            post_reply_max_age_days,
            post_reply_max_points,
            post_subject_max_length,
            post_content_max_bytes,
            image_directory,
            image_upload_max_bytes,
            sessions: SessionStore::new(auth.session_ttl, auth.session_cookie_secure),
            login_hash_permits: Arc::new(Semaphore::new(auth.login_max_concurrent_hashes.max(1))),
        }
    }
}
