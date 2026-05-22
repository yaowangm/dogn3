use crate::cache::RedisCache;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: Option<RedisCache>,
    pub site_name: String,
}

impl AppState {
    pub fn new(pool: PgPool, cache: Option<RedisCache>, site_name: String) -> Self {
        Self {
            pool,
            cache,
            site_name,
        }
    }
}
