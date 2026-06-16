use abt_web::{config, routes, state::AppState};
use time::Duration;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config = config::Config::from_env();
    let state = AppState::new(&config).await?;
    let addr = format!("{}:{}", config.host, config.port);

    let session_layer = SessionManagerLayer::new(state.session_store.clone())
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::hours(
            state.jwt_expiration_hours as i64,
        )));

    let app = routes::router(state)
        .fallback_service(ServeDir::new("static"))
        .layer(session_layer);

    tracing::info!("HTTP listening on http://{addr}");
    axum_server::bind(addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
