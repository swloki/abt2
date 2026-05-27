use crate::config::Config;
use sqlx::PgPool;
use std::sync::Arc;
use tower_sessions_file_store::FileSessionStorage;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub session_store: FileSessionStorage,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connection)
            .connect(&config.database_url)
            .await?;

        tracing::info!("Database pool initialized (max {} connections)", config.max_connection);

        let session_store = FileSessionStorage::new_in_folder(std::path::PathBuf::from(&config.session_dir));

        tracing::info!("File session store initialized at: {}", config.session_dir);

        Ok(Self {
            pool,
            jwt_secret: config.jwt_secret.clone(),
            jwt_expiration_hours: config.jwt_expiration_hours,
            session_store,
        })
    }

    pub fn auth_service(&self) -> impl abt_core::shared::identity::AuthService {
        use abt_core::shared::identity::implt::AuthServiceImpl;
        AuthServiceImpl::new(Arc::new(self.pool.clone()), self.jwt_secret.clone())
    }

    pub fn customer_service(&self) -> impl abt_core::master_data::customer::CustomerService {
        abt_core::master_data::customer::new_customer_service(self.pool.clone())
    }
}
