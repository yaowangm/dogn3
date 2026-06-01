use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
    pub board_page_size: i64,
    pub post_reply_max_age_days: i32,
    pub post_reply_max_points: i32,
    pub post_reply_allow_self_points: bool,
    pub post_subject_max_length: usize,
    pub post_content_max_bytes: usize,
    pub cache_enabled: bool,
    pub redis_url: String,
    pub redis_key_prefix: String,
    pub redis_default_ttl: Duration,
    pub site_name: String,
    pub image_directory: PathBuf,
    pub image_upload_max_bytes: usize,
    pub session_ttl: Duration,
    pub session_cookie_secure: bool,
    pub login_max_concurrent_hashes: usize,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_env_reader(|key| env::var(key))
    }

    fn from_env_reader(
        get_var: impl Fn(&str) -> Result<String, env::VarError>,
    ) -> anyhow::Result<Self> {
        let bind_addr = get_var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
            .parse()?;
        let database_url = get_var("DATABASE_URL")?;
        let database_max_connections = get_var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()?;
        let board_page_size = get_var("BOARD_PAGE_SIZE")
            .unwrap_or_else(|_| "50".to_string())
            .parse::<i64>()?
            .max(1);
        let post_reply_max_age_days = get_var("POST_REPLY_MAX_AGE_DAYS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<i32>()?;
        anyhow::ensure!(
            post_reply_max_age_days > 0,
            "POST_REPLY_MAX_AGE_DAYS must be greater than 0"
        );
        let post_reply_max_points = get_var("POST_REPLY_MAX_POINTS")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<i32>()?;
        anyhow::ensure!(
            post_reply_max_points >= 0,
            "POST_REPLY_MAX_POINTS must not be negative"
        );
        let post_reply_allow_self_points = parse_bool(
            &get_var("POST_REPLY_ALLOW_SELF_POINTS").unwrap_or_else(|_| "true".to_string()),
        )?;
        let post_subject_max_length = get_var("POST_SUBJECT_MAX_LENGTH")
            .unwrap_or_else(|_| "50".to_string())
            .parse::<usize>()?;
        anyhow::ensure!(
            post_subject_max_length > 0,
            "POST_SUBJECT_MAX_LENGTH must be greater than 0"
        );
        let post_content_max_bytes = get_var("POST_CONTENT_MAX_BYTES")
            .unwrap_or_else(|_| "131072".to_string())
            .parse::<usize>()?;
        anyhow::ensure!(
            post_content_max_bytes > 0,
            "POST_CONTENT_MAX_BYTES must be greater than 0"
        );
        let cache_enabled =
            parse_bool(&get_var("CACHE_ENABLED").unwrap_or_else(|_| "true".to_string()))?;
        let redis_url =
            get_var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let redis_key_prefix = get_var("REDIS_KEY_PREFIX")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "dogn3".to_string());
        let redis_default_ttl = Duration::from_secs(
            get_var("REDIS_DEFAULT_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,
        );
        let site_name = get_var("SITE_NAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Dogn".to_string());
        let image_directory = get_var("IMAGE_DIRECTORY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("images"));
        let image_upload_max_bytes = get_var("IMAGE_UPLOAD_MAX_BYTES")
            .unwrap_or_else(|_| "2097152".to_string())
            .parse::<usize>()?;
        anyhow::ensure!(
            (1..=10 * 1024 * 1024).contains(&image_upload_max_bytes),
            "IMAGE_UPLOAD_MAX_BYTES must be between 1 and 10485760"
        );
        let session_ttl = Duration::from_secs(
            get_var("SESSION_TTL_SECONDS")
                .unwrap_or_else(|_| "604800".to_string())
                .parse()?,
        );
        let session_cookie_secure =
            parse_bool(&get_var("SESSION_COOKIE_SECURE").unwrap_or_else(|_| "false".to_string()))?;
        let login_max_concurrent_hashes = get_var("LOGIN_MAX_CONCURRENT_HASHES")
            .unwrap_or_else(|_| "2".to_string())
            .parse::<usize>()?
            .max(1);

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
            board_page_size,
            post_reply_max_age_days,
            post_reply_max_points,
            post_reply_allow_self_points,
            post_subject_max_length,
            post_content_max_bytes,
            cache_enabled,
            redis_url,
            redis_key_prefix,
            redis_default_ttl,
            site_name,
            image_directory,
            image_upload_max_bytes,
            session_ttl,
            session_cookie_secure,
            login_max_concurrent_hashes,
        })
    }
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("invalid boolean value: {value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use std::{collections::HashMap, env};

    fn config_from(values: &[(&str, &str)]) -> anyhow::Result<AppConfig> {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();

        AppConfig::from_env_reader(|key| values.get(key).cloned().ok_or(env::VarError::NotPresent))
    }

    #[test]
    fn uses_defaults_for_optional_values() {
        let config = config_from(&[("DATABASE_URL", "postgres:///dogn_test")]).unwrap();

        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(config.database_url, "postgres:///dogn_test");
        assert_eq!(config.database_max_connections, 5);
        assert_eq!(config.board_page_size, 50);
        assert_eq!(config.post_reply_max_age_days, 10);
        assert_eq!(config.post_reply_max_points, 100);
        assert!(config.post_reply_allow_self_points);
        assert_eq!(config.post_subject_max_length, 50);
        assert_eq!(config.post_content_max_bytes, 131_072);
        assert!(config.cache_enabled);
        assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
        assert_eq!(config.redis_key_prefix, "dogn3");
        assert_eq!(config.redis_default_ttl.as_secs(), 300);
        assert_eq!(config.site_name, "Dogn");
        assert_eq!(config.image_directory, std::path::PathBuf::from("images"));
        assert_eq!(config.image_upload_max_bytes, 2_097_152);
        assert_eq!(config.session_ttl.as_secs(), 604_800);
        assert!(!config.session_cookie_secure);
        assert_eq!(config.login_max_concurrent_hashes, 2);
    }

    #[test]
    fn trims_site_name() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("SITE_NAME", "  My Forum  "),
        ])
        .unwrap();

        assert_eq!(config.site_name, "My Forum");
    }

    #[test]
    fn reads_image_directory() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("IMAGE_DIRECTORY", "  /srv/dogn/images  "),
        ])
        .unwrap();

        assert_eq!(
            config.image_directory,
            std::path::PathBuf::from("/srv/dogn/images")
        );
    }

    #[test]
    fn reads_image_upload_limit() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("IMAGE_UPLOAD_MAX_BYTES", "2048"),
        ])
        .unwrap();

        assert_eq!(config.image_upload_max_bytes, 2_048);
    }

    #[test]
    fn rejects_image_upload_limit_above_route_ceiling() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("IMAGE_UPLOAD_MAX_BYTES", "10485761"),
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn reads_session_settings() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("SESSION_TTL_SECONDS", "1800"),
            ("SESSION_COOKIE_SECURE", "true"),
            ("LOGIN_MAX_CONCURRENT_HASHES", "4"),
        ])
        .unwrap();

        assert_eq!(config.session_ttl.as_secs(), 1_800);
        assert!(config.session_cookie_secure);
        assert_eq!(config.login_max_concurrent_hashes, 4);
    }

    #[test]
    fn rejects_invalid_bind_addr() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("BIND_ADDR", "not-a-socket"),
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn reads_redis_settings() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("CACHE_ENABLED", "false"),
            ("REDIS_URL", "redis://localhost:6379/1"),
            ("REDIS_KEY_PREFIX", "  test-prefix  "),
            ("REDIS_DEFAULT_TTL_SECONDS", "60"),
        ])
        .unwrap();

        assert!(!config.cache_enabled);
        assert_eq!(config.redis_url, "redis://localhost:6379/1");
        assert_eq!(config.redis_key_prefix, "test-prefix");
        assert_eq!(config.redis_default_ttl.as_secs(), 60);
    }

    #[test]
    fn reads_board_page_size() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("BOARD_PAGE_SIZE", "25"),
        ])
        .unwrap();

        assert_eq!(config.board_page_size, 25);
    }

    #[test]
    fn reads_reply_age_limit() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_REPLY_MAX_AGE_DAYS", "30"),
        ])
        .unwrap();

        assert_eq!(config.post_reply_max_age_days, 30);
    }

    #[test]
    fn rejects_non_positive_reply_age_limit() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_REPLY_MAX_AGE_DAYS", "0"),
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn reads_reply_point_limit() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_REPLY_MAX_POINTS", "25"),
        ])
        .unwrap();

        assert_eq!(config.post_reply_max_points, 25);
    }

    #[test]
    fn rejects_negative_reply_point_limit() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_REPLY_MAX_POINTS", "-1"),
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn reads_reply_self_point_policy() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_REPLY_ALLOW_SELF_POINTS", "false"),
        ])
        .unwrap();

        assert!(!config.post_reply_allow_self_points);
    }

    #[test]
    fn reads_post_text_limits() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("POST_SUBJECT_MAX_LENGTH", "64"),
            ("POST_CONTENT_MAX_BYTES", "4096"),
        ])
        .unwrap();

        assert_eq!(config.post_subject_max_length, 64);
        assert_eq!(config.post_content_max_bytes, 4_096);
    }

    #[test]
    fn rejects_zero_post_text_limits() {
        assert!(
            config_from(&[
                ("DATABASE_URL", "postgres:///dogn_test"),
                ("POST_SUBJECT_MAX_LENGTH", "0"),
            ])
            .is_err()
        );
        assert!(
            config_from(&[
                ("DATABASE_URL", "postgres:///dogn_test"),
                ("POST_CONTENT_MAX_BYTES", "0"),
            ])
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_cache_enabled_value() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn_test"),
            ("CACHE_ENABLED", "maybe"),
        ]);

        assert!(result.is_err());
    }
}
