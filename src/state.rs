use crate::cache::RedisCache;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
    pub board_page_size: i64,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        cache: Option<RedisCache>,
        site_name: String,
        board_page_size: i64,
    ) -> Self {
        Self {
            pool,
            cache,
            site_name,
            board_page_size,
        }
    }
}
