//! gRPC Server 配置 - 使用环境变量

use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct Config {
    pub grpc_host: String,
    pub grpc_port: u16,
    pub database_url: String,
    pub max_connection: u32,
    pub upload_temp_dir: String,
}

static CONFIG: LazyLock<Config> = LazyLock::new(|| Config {
    grpc_host: std::env::var("GRPC_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
    grpc_port: std::env::var("GRPC_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8001),
    database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
    max_connection: std::env::var("MAX_CONNECTION")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20),
    upload_temp_dir: std::env::var("UPLOAD_TEMP_DIR").expect("UPLOAD_TEMP_DIR must be set"),
});

pub fn get_config() -> &'static Config {
    &CONFIG
}
