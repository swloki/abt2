pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub max_connection: u32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("WEB_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("WEB_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8000),
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL is required"),
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET is required"),
            jwt_expiration_hours: std::env::var("JWT_EXPIRATION_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(72),
            max_connection: std::env::var("MAX_CONNECTION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
        }
    }
}
