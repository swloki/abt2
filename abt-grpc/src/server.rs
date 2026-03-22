//! gRPC Server 配置和启动

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

// Re-export config types
pub use crate::config::{get_config, Config};

// Global application state
static APP_STATE: OnceCell<Arc<AppState>> = OnceCell::const_new();

pub struct AppState {
    abt_context: &'static abt::AppContext,
}

impl AppState {
    /// Initialize the global application state using TOML config
    pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
        let config = get_config();

        // Create database pool from TOML config
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connection)
            .connect(&config.database_url)
            .await?;

        // Initialize abt context with the pool
        abt::init_context_with_pool(pool).await;

        // Context is now initialized, get reference
        let ctx = abt::get_context().await;
        let state = Arc::new(AppState { abt_context: ctx });

        APP_STATE
            .set(state)
            .map_err(|_| "AppState already initialized")?;

        Ok(())
    }

    /// Get the global application state
    pub async fn get() -> Arc<AppState> {
        APP_STATE.get().expect("AppState not initialized").clone()
    }

    pub fn product_service(&self) -> impl abt::ProductService {
        abt::get_product_service(self.abt_context)
    }

    pub fn term_service(&self) -> impl abt::TermService {
        abt::get_term_service(self.abt_context)
    }

    pub fn bom_service(&self) -> impl abt::BomService {
        abt::get_bom_service(self.abt_context)
    }

    pub fn warehouse_service(&self) -> impl abt::WarehouseService {
        abt::get_warehouse_service(self.abt_context)
    }

    pub fn location_service(&self) -> impl abt::LocationService {
        abt::get_location_service(self.abt_context)
    }

    pub fn inventory_service(&self) -> impl abt::InventoryService {
        abt::get_inventory_service(self.abt_context)
    }

    pub fn excel_service(&self) -> impl abt::ProductExcelService {
        abt::get_product_excel_service(self.abt_context)
    }

    pub fn price_service(&self) -> impl abt::ProductPriceService {
        abt::get_product_price_service(self.abt_context)
    }

    pub fn labor_process_service(&self) -> impl abt::LaborProcessService {
        abt::get_labor_process_service(self.abt_context)
    }

    pub fn user_service(&self) -> impl abt::UserService {
        abt::get_user_service(self.abt_context)
    }

    pub fn role_service(&self) -> impl abt::RoleService {
        abt::get_role_service(self.abt_context)
    }

    pub fn permission_service(&self) -> impl abt::PermissionService {
        abt::get_permission_service(self.abt_context)
    }

    pub async fn begin_transaction(&self) -> anyhow::Result<sqlx::Transaction<'static, sqlx::Postgres>> {
        self.abt_context.begin_transaction().await
    }

    pub fn pool(&self) -> sqlx::PgPool {
        self.abt_context.pool().clone()
    }
}

pub async fn start_server(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    AppState::init().await?;

    let reflection_service = Builder::configure()
        .build_v1()
        .expect("Failed to build reflection service");

    use crate::handlers::{
        AbtBomServiceServer, AbtExcelServiceServer, AbtInventoryServiceServer,
        AbtLocationServiceServer, AbtPriceServiceServer, AbtProductServiceServer,
        AbtTermServiceServer, AbtWarehouseServiceServer,
    };

    Server::builder()
        .add_service(reflection_service)
        .add_service(AbtProductServiceServer::new(
            crate::handlers::product::ProductHandler::new(),
        ))
        .add_service(AbtTermServiceServer::new(
            crate::handlers::term::TermHandler::new(),
        ))
        .add_service(AbtBomServiceServer::new(
            crate::handlers::bom::BomHandler::new(),
        ))
        .add_service(AbtWarehouseServiceServer::new(
            crate::handlers::warehouse::WarehouseHandler::new(),
        ))
        .add_service(AbtLocationServiceServer::new(
            crate::handlers::location::LocationHandler::new(),
        ))
        .add_service(AbtInventoryServiceServer::new(
            crate::handlers::inventory::InventoryHandler::new(),
        ))
        .add_service(AbtExcelServiceServer::new(
            crate::handlers::excel::ExcelHandler::new(),
        ))
        .add_service(AbtPriceServiceServer::new(
            crate::handlers::price::PriceHandler::new(),
        ))
        .serve(addr)
        .await?;

    Ok(())
}
