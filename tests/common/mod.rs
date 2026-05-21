use dogn3::{build_router, state::AppState};
use sqlx::PgPool;

pub async fn test_pool() -> Option<PgPool> {
    let database_url = match std::env::var("TEST_DATABASE_URL") {
        Ok(database_url) => database_url,
        Err(_) => return None,
    };

    Some(
        PgPool::connect(&database_url)
            .await
            .expect("failed to connect to test database"),
    )
}

pub fn test_app(pool: PgPool) -> axum::Router {
    build_router(AppState::new(pool, "Test Forum".to_string()))
}
