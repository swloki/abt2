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

        // Create database pool from config
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

    pub fn price_service(&self) -> impl abt::ProductPriceService {
        abt::get_product_price_service(self.abt_context)
    }

    pub fn labor_process_service(&self) -> impl abt::LaborProcessService {
        abt::get_labor_process_service(self.abt_context)
    }

    pub fn labor_process_dict_service(&self) -> impl abt::LaborProcessDictService {
        abt::get_labor_process_dict_service(self.abt_context)
    }

    pub fn routing_service(&self) -> impl abt::RoutingService {
        abt::get_routing_service(self.abt_context)
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

    pub fn department_service(&self) -> impl abt::DepartmentService {
        abt::get_department_service(self.abt_context)
    }

    pub fn bom_category_service(&self) -> impl abt::BomCategoryService {
        abt::get_bom_category_service(self.abt_context)
    }

    pub fn inventory_cascade_service(&self) -> impl abt::InventoryCascadeService {
        abt::get_inventory_cascade_service(self.abt_context)
    }

    pub fn auth_service(&self) -> impl abt::AuthService {
        let config = get_config();
        let resources = abt::collect_all_resources();
        abt::get_auth_service(
            self.pool(),
            config.jwt_secret.clone(),
            config.jwt_expiration_hours,
            resources,
        )
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
        AbtLaborProcessServiceServer, AbtLaborProcessDictServiceServer, AbtLocationServiceServer, AbtPriceServiceServer,
        AbtProductServiceServer, AbtRoutingServiceServer, AbtTermServiceServer, AbtWarehouseServiceServer,
        AuthServiceServer, AbtBomCategoryServiceServer, DepartmentServiceServer,
        PermissionServiceServer, RoleServiceServer, UserServiceServer,
    };
    use crate::interceptors::auth_interceptor;

    // AuthService 不加 interceptor (Login 不需要 JWT)
    Server::builder()
        .add_service(reflection_service)
        .add_service(AuthServiceServer::new(
            crate::handlers::auth::AuthHandler::new(),
        ))
        .add_service(AbtProductServiceServer::with_interceptor(
            crate::handlers::product::ProductHandler::new(), auth_interceptor,
        ))
        .add_service(AbtTermServiceServer::with_interceptor(
            crate::handlers::term::TermHandler::new(), auth_interceptor,
        ))
        .add_service(AbtBomServiceServer::with_interceptor(
            crate::handlers::bom::BomHandler::new(), auth_interceptor,
        ))
        .add_service(AbtWarehouseServiceServer::with_interceptor(
            crate::handlers::warehouse::WarehouseHandler::new(), auth_interceptor,
        ))
        .add_service(AbtLocationServiceServer::with_interceptor(
            crate::handlers::location::LocationHandler::new(), auth_interceptor,
        ))
        .add_service(AbtInventoryServiceServer::with_interceptor(
            crate::handlers::inventory::InventoryHandler::new(), auth_interceptor,
        ))
        .add_service(AbtExcelServiceServer::with_interceptor(
            crate::handlers::excel::ExcelHandler::new(), auth_interceptor,
        ))
        .add_service(AbtPriceServiceServer::with_interceptor(
            crate::handlers::price::PriceHandler::new(), auth_interceptor,
        ))
        .add_service(UserServiceServer::with_interceptor(
            crate::handlers::user::UserHandler::new(), auth_interceptor,
        ))
        .add_service(RoleServiceServer::with_interceptor(
            crate::handlers::role::RoleHandler::new(), auth_interceptor,
        ))
        .add_service(PermissionServiceServer::with_interceptor(
            crate::handlers::permission::PermissionHandler::new(), auth_interceptor,
        ))
        .add_service(DepartmentServiceServer::with_interceptor(
            crate::handlers::department::DepartmentHandler::new(), auth_interceptor,
        ))
        .add_service(AbtBomCategoryServiceServer::with_interceptor(
            crate::handlers::bom_category::BomCategoryHandler::new(), auth_interceptor,
        ))
        .add_service(AbtLaborProcessServiceServer::with_interceptor(
            crate::handlers::labor_process::LaborProcessHandler::new(), auth_interceptor,
        ))
        .add_service(AbtLaborProcessDictServiceServer::with_interceptor(
            crate::handlers::labor_process_dict::LaborProcessDictHandler::new(), auth_interceptor,
        ))
        .add_service(AbtRoutingServiceServer::with_interceptor(
            crate::handlers::routing::RoutingHandler::new(), auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
