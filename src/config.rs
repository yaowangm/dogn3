use std::{env, net::SocketAddr, time::Duration};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
    pub board_page_size: i64,
    pub cache_enabled: bool,
    pub redis_url: String,
    pub redis_key_prefix: String,
    pub redis_default_ttl: Duration,
    pub site_name: String,
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

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
            board_page_size,
            cache_enabled,
            redis_url,
            redis_key_prefix,
            redis_default_ttl,
            site_name,
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
        let config = config_from(&[("DATABASE_URL", "postgres:///dogn3_test")]).unwrap();

        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(config.database_url, "postgres:///dogn3_test");
        assert_eq!(config.database_max_connections, 5);
        assert_eq!(config.board_page_size, 50);
        assert!(config.cache_enabled);
        assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
        assert_eq!(config.redis_key_prefix, "dogn3");
        assert_eq!(config.redis_default_ttl.as_secs(), 300);
        assert_eq!(config.site_name, "Dogn");
    }

    #[test]
    fn trims_site_name() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn3_test"),
            ("SITE_NAME", "  My Forum  "),
        ])
        .unwrap();

        assert_eq!(config.site_name, "My Forum");
    }

    #[test]
    fn rejects_invalid_bind_addr() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn3_test"),
            ("BIND_ADDR", "not-a-socket"),
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn reads_redis_settings() {
        let config = config_from(&[
            ("DATABASE_URL", "postgres:///dogn3_test"),
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
            ("DATABASE_URL", "postgres:///dogn3_test"),
            ("BOARD_PAGE_SIZE", "25"),
        ])
        .unwrap();

        assert_eq!(config.board_page_size, 25);
    }

    #[test]
    fn rejects_invalid_cache_enabled_value() {
        let result = config_from(&[
            ("DATABASE_URL", "postgres:///dogn3_test"),
            ("CACHE_ENABLED", "maybe"),
        ]);

        assert!(result.is_err());
    }
}
