mod auth;
mod components;
mod config;
mod errors;
mod layout;
mod routes;
mod state;

use state::AppState;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
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

    tracing::info!("Starting abt-web2 on http://{addr}");

    let app = routes::router(state)
        .fallback_service(ServeDir::new("static"))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
