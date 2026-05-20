use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub site_name: String,
}

impl AppState {
    pub fn new(pool: PgPool, site_name: String) -> Self {
        Self { pool, site_name }
    }
}
