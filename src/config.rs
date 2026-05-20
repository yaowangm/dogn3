use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
            .parse()?;
        let database_url = env::var("DATABASE_URL")?;
        let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()?;

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
        })
    }
}
