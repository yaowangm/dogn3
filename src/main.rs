use dogn3::{
    build_router,
    cache::RedisCache,
    config::AppConfig,
    rate_limit::RateLimitConfig,
    state::{AppState, AuthRuntimeConfig, PasswordResetConfig},
};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await?;
    let cache = if config.cache_enabled {
        let cache = RedisCache::new(
            &config.redis_url,
            config.redis_key_prefix.clone(),
            config.redis_default_ttl,
        )?;
        cache.ping().await?;
        Some(cache)
    } else {
        tracing::info!("cache disabled");
        None
    };

    let app = build_router(AppState::new(
        pool,
        cache,
        config.site_name.clone(),
        config.board_page_size,
        config.post_reply_max_age_days,
        config.post_reply_max_points,
        config.new_user_initial_points,
        config.root_post_regular_award_points,
        config.root_post_forward_award_points,
        config.root_post_original_award_points,
        config.post_subject_max_length,
        config.post_content_max_bytes,
        config.post_signature_max_bytes,
        config.image_directory.clone(),
        config.image_upload_max_bytes,
        AuthRuntimeConfig {
            session_ttl: config.session_ttl,
            session_cookie_secure: config.session_cookie_secure,
            login_max_concurrent_hashes: config.login_max_concurrent_hashes,
        },
        PasswordResetConfig {
            enabled: config.password_reset_enabled,
            sendmail_path: config.sendmail_path.clone(),
            mail_from: config.mail_from.clone(),
            public_site_url: config.public_site_url.clone(),
            ttl: config.password_reset_ttl,
        },
        RateLimitConfig {
            enabled: config.rate_limit_enabled,
            backend: config.rate_limit_backend,
            login_fail_window: config.login_fail_window,
            login_fail_max_per_user: config.login_fail_max_per_user,
            login_fail_max_per_ip: config.login_fail_max_per_ip,
            login_fail_lock: config.login_fail_lock,
            password_reset_window: config.password_reset_window,
            password_reset_max_per_email: config.password_reset_max_per_email,
            password_reset_max_per_ip: config.password_reset_max_per_ip,
            password_reset_confirm_window: config.password_reset_confirm_window,
            password_reset_confirm_max_per_ip: config.password_reset_confirm_max_per_ip,
        },
    ));
    let listener = TcpListener::bind(config.bind_addr).await?;

    tracing::info!(address = %config.bind_addr, "server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dogn3=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install terminate signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
