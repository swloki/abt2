//! gRPC Server 配置

use std::fs;
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub grpc: GrpcConfig,
    pub database_url: String,
    pub max_connection: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrpcConfig {
    pub host: String,
    pub port: u16,
}

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let config_str = fs::read_to_string("config.toml").expect("Failed to read config.toml");
    let mut config: Config = toml::from_str(&config_str).expect("Failed to parse config.toml");

    // 环境变量可覆盖 database_url
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        config.database_url = db_url;
    }

    config
});

pub fn get_config() -> &'static Config {
    &CONFIG
}
