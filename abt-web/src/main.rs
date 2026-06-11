mod auth;
mod components;
mod config;
mod errors;
mod layout;
mod pages;
mod permissions;
mod routes;
mod state;
mod utils;

use state::AppState;
use time::Duration;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
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
        .layer(session_layer)
        .layer(TraceLayer::new_for_http());

    let tls_config = load_or_generate_tls().await?;
    tracing::info!("HTTPS listening on https://{addr}");
    axum_server::bind_rustls(addr.parse()?, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn load_or_generate_tls() -> Result<axum_server::tls_rustls::RustlsConfig, Box<dyn std::error::Error>> {
    use rcgen::{CertificateParams, DnType, KeyPair};

    let cert_path = std::path::Path::new("cert.der");
    let key_path = std::path::Path::new("key.der");

    if cert_path.exists() && key_path.exists() {
        tracing::info!("Loading existing TLS certificate from disk");
        let cert_der = std::fs::read(cert_path)?;
        let key_der = std::fs::read(key_path)?;
        return Ok(axum_server::tls_rustls::RustlsConfig::from_der(
            vec![cert_der], key_der,
        ).await?);
    }

    tracing::info!("Generating new self-signed TLS certificate");
    let mut params = CertificateParams::default();
    params.distinguished_name.push(DnType::CommonName, "ABT Dev Server");
    params.distinguished_name.push(DnType::OrganizationName, "ABT");
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_der = cert.der().to_vec();
    let key_der = key_pair.serialize_der();

    std::fs::write(cert_path, &cert_der)?;
    std::fs::write(key_path, &key_der)?;
    tracing::info!("TLS certificate saved to cert.der / key.der");

    Ok(axum_server::tls_rustls::RustlsConfig::from_der(
        vec![cert_der], key_der,
    ).await?)
}
