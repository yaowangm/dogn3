use crate::cache::RedisCache;
use sqlx::PgPool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
    pub board_page_size: i64,
    pub image_directory: PathBuf,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        cache: Option<RedisCache>,
        site_name: String,
        board_page_size: i64,
        image_directory: PathBuf,
    ) -> Self {
        Self {
            pool,
            cache,
            site_name,
            board_page_size,
            image_directory,
        }
    }
}
