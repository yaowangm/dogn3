use anyhow::Context;
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
    log_startup_config(&config);

    let sanitized_database_url = sanitized_connection_url(&config.database_url);
    tracing::info!(
        database_url = %sanitized_database_url,
        max_connections = config.database_max_connections,
        "connecting to PostgreSQL"
    );
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await
        .with_context(|| {
            format!(
                "failed to connect to PostgreSQL at {}{}",
                sanitized_database_url,
                docker_loopback_hint(&config.database_url)
            )
        })?;
    tracing::info!("connected to PostgreSQL");

    let cache = if config.cache_enabled {
        let sanitized_redis_url = sanitized_connection_url(&config.redis_url);
        tracing::info!(
            redis_url = %sanitized_redis_url,
            key_prefix = %config.redis_key_prefix,
            default_ttl_seconds = config.redis_default_ttl.as_secs(),
            "connecting to Redis"
        );
        let cache = RedisCache::new(
            &config.redis_url,
            config.redis_key_prefix.clone(),
            config.redis_default_ttl,
        )
        .with_context(|| format!("failed to create Redis client for {sanitized_redis_url}"))?;
        cache.ping().await.with_context(|| {
            format!(
                "failed to ping Redis at {}{}",
                sanitized_redis_url,
                docker_loopback_hint(&config.redis_url)
            )
        })?;
        tracing::info!("connected to Redis");
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
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP listener at {}", config.bind_addr))?;

    tracing::info!(address = %config.bind_addr, "server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

fn log_startup_config(config: &AppConfig) {
    tracing::info!(
        site_name = %config.site_name,
        bind_addr = %config.bind_addr,
        image_directory = %config.image_directory.display(),
        cache_enabled = config.cache_enabled,
        rate_limit_enabled = config.rate_limit_enabled,
        rate_limit_backend = ?config.rate_limit_backend,
        password_reset_enabled = config.password_reset_enabled,
        session_ttl_seconds = config.session_ttl.as_secs(),
        "loaded runtime configuration"
    );
}

fn sanitized_connection_url(url: &str) -> String {
    let without_password = match url.find("://") {
        Some(scheme_end) => {
            let authority_start = scheme_end + 3;
            match url[authority_start..].find('@') {
                Some(relative_at) => {
                    let at = authority_start + relative_at;
                    let authority = &url[authority_start..at];
                    match authority.rfind(':') {
                        Some(colon) => {
                            format!(
                                "{}{}:***{}",
                                &url[..authority_start],
                                &authority[..colon],
                                &url[at..]
                            )
                        }
                        None => url.to_string(),
                    }
                }
                None => url.to_string(),
            }
        }
        None => url.to_string(),
    };

    redact_query_passwords(&without_password)
}

fn redact_query_passwords(url: &str) -> String {
    url.split('&')
        .map(|segment| match segment.split_once('=') {
            Some((key, _)) => {
                let normalized_key = key.to_ascii_lowercase();
                if normalized_key.ends_with("password") || normalized_key.ends_with("pass") {
                    format!("{key}=***")
                } else {
                    segment.to_string()
                }
            }
            _ => segment.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn docker_loopback_hint(url: &str) -> &'static str {
    if contains_loopback_host(url) {
        "; configured host is localhost/127.0.0.1, which points inside a Docker bridge container. Use host.docker.internal, host networking, or a Docker-network service name when running in Docker."
    } else {
        ""
    }
}

fn contains_loopback_host(url: &str) -> bool {
    url.contains("@localhost")
        || url.contains("@127.0.0.1")
        || url.contains("://localhost")
        || url.contains("://127.0.0.1")
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

#[cfg(test)]
mod tests {
    use super::{docker_loopback_hint, sanitized_connection_url};

    #[test]
    fn sanitized_connection_url_redacts_authority_password() {
        assert_eq!(
            sanitized_connection_url("postgres://wy:secret@localhost:5432/dogn"),
            "postgres://wy:***@localhost:5432/dogn"
        );
    }

    #[test]
    fn sanitized_connection_url_redacts_query_password() {
        assert_eq!(
            sanitized_connection_url("redis://localhost:6379?password=secret"),
            "redis://localhost:6379?password=***"
        );
        assert_eq!(
            sanitized_connection_url("redis://localhost:6379?PASSWORD=secret"),
            "redis://localhost:6379?PASSWORD=***"
        );
    }

    #[test]
    fn docker_loopback_hint_detects_localhost_urls() {
        assert!(
            docker_loopback_hint("postgres://wy:secret@localhost:5432/dogn").contains("Docker")
        );
        assert!(
            docker_loopback_hint("postgres://wy:secret@host.docker.internal:5432/dogn").is_empty()
        );
    }
}
