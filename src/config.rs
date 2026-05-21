use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
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
        let site_name = get_var("SITE_NAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Dogn".to_string());

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
            site_name,
        })
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
}
