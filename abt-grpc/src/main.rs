//! ABT gRPC Server

use abt_grpc::server::{get_config, start_server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = get_config();
    let addr: std::net::SocketAddr =
        format!("{}:{}", config.grpc_host, config.grpc_port).parse()?;

    tracing::info!("Starting ABT gRPC server on {}", addr);
    start_server(addr).await
}
