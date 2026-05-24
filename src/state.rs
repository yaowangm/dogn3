use crate::{auth::SessionStore, cache::RedisCache};
use sqlx::PgPool;
use std::{path::PathBuf, time::Duration};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
    pub board_page_size: i64,
    pub image_directory: PathBuf,
    pub sessions: SessionStore,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        cache: Option<RedisCache>,
        site_name: String,
        board_page_size: i64,
        image_directory: PathBuf,
        session_ttl: Duration,
        session_cookie_secure: bool,
    ) -> Self {
        Self {
            pool,
            cache,
            site_name,
            board_page_size,
            image_directory,
            sessions: SessionStore::new(session_ttl, session_cookie_secure),
        }
    }
}
