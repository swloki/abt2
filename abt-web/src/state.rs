use std::sync::Arc;

use crate::config::Config;
use abt_core::shared::identity::RolePermissionCache;
use abt_core::shared::types::{PgPool, PgPoolOptions};
use tower_sessions_file_store::FileSessionStorage;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub session_store: FileSessionStorage,
    pub permission_cache: Arc<RolePermissionCache>,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connection)
            .connect(&config.database_url)
            .await?;

        tracing::info!(
            "Database pool initialized (max {} connections)",
            config.max_connection
        );

        let session_store =
            FileSessionStorage::new_in_folder(std::path::PathBuf::from(&config.session_dir));

        tracing::info!("File session store initialized at: {}", config.session_dir);

        let permission_cache = Arc::new(RolePermissionCache::new(pool.clone()));
        permission_cache.load(&pool).await?;
        tracing::info!("Permission cache loaded");

        Ok(Self {
            pool,
            jwt_secret: config.jwt_secret.clone(),
            jwt_expiration_hours: config.jwt_expiration_hours,
            session_store,
            permission_cache,
        })
    }

    pub fn auth_service(&self) -> impl abt_core::shared::identity::AuthService {
        use abt_core::shared::identity::implt::AuthServiceImpl;
        AuthServiceImpl::new(self.pool.clone(), self.jwt_secret.clone())
    }

    pub fn customer_service(&self) -> impl abt_core::master_data::customer::CustomerService {
        abt_core::master_data::customer::new_customer_service(self.pool.clone())
    }

    pub fn quotation_service(&self) -> impl abt_core::sales::quotation::QuotationService {
        abt_core::sales::quotation::new_quotation_service(self.pool.clone())
    }

    pub fn product_service(&self) -> impl abt_core::master_data::product::ProductService {
        abt_core::master_data::product::new_product_service(self.pool.clone())
    }

    pub fn sales_order_service(&self) -> impl abt_core::sales::sales_order::SalesOrderService {
        abt_core::sales::sales_order::new_sales_order_service(self.pool.clone())
    }

    pub fn shipping_service(
        &self,
    ) -> impl abt_core::sales::shipping_request::ShippingRequestService {
        abt_core::sales::shipping_request::new_shipping_request_service(self.pool.clone())
    }

    pub fn warehouse_service(&self) -> impl abt_core::wms::warehouse::WarehouseService {
        abt_core::wms::warehouse::new_warehouse_service(self.pool.clone())
    }

    #[allow(dead_code)]
    pub fn bom_query_service(&self) -> impl abt_core::master_data::bom::BomQueryService {
        abt_core::master_data::bom::new_bom_query_service(self.pool.clone())
    }

    #[allow(dead_code)]
    pub fn bom_command_service(&self) -> impl abt_core::master_data::bom::BomCommandService {
        abt_core::master_data::bom::new_bom_command_service(self.pool.clone())
    }

    #[allow(dead_code)]
    pub fn bom_node_service(&self) -> impl abt_core::master_data::bom::BomNodeService {
        abt_core::master_data::bom::new_bom_node_service(self.pool.clone())
    }

    #[allow(dead_code)]
    pub fn routing_service(&self) -> impl abt_core::master_data::routing::RoutingService {
        abt_core::master_data::routing::new_routing_service(self.pool.clone())
    }

    pub fn sales_return_service(&self) -> impl abt_core::sales::sales_return::SalesReturnService {
        abt_core::sales::sales_return::new_sales_return_service(self.pool.clone())
    }

    pub fn reconciliation_service(
        &self,
    ) -> impl abt_core::sales::reconciliation::ReconciliationService {
        abt_core::sales::reconciliation::new_reconciliation_service(self.pool.clone())
    }

    pub fn user_service(&self) -> impl abt_core::shared::identity::UserService {
        abt_core::shared::identity::new_user_service(self.pool.clone())
    }

    pub fn permission_service(
        &self,
    ) -> impl abt_core::shared::identity::PermissionService {
        abt_core::shared::identity::implt::PermissionServiceImpl::new(
            self.permission_cache.clone(),
        )
    }

    // ── Purchase (SRM) Services ──

    pub fn supplier_service(&self) -> impl abt_core::master_data::supplier::SupplierService {
        abt_core::master_data::supplier::new_supplier_service(self.pool.clone())
    }

    pub fn purchase_quotation_service(
        &self,
    ) -> impl abt_core::purchase::quotation::PurchaseQuotationService {
        abt_core::purchase::quotation::new_purchase_quotation_service(self.pool.clone())
    }

    pub fn purchase_order_service(
        &self,
    ) -> impl abt_core::purchase::order::PurchaseOrderService {
        abt_core::purchase::order::new_purchase_order_service(self.pool.clone())
    }

    pub fn purchase_return_service(
        &self,
    ) -> impl abt_core::purchase::return_order::PurchaseReturnService {
        abt_core::purchase::return_order::new_purchase_return_service(self.pool.clone())
    }

    pub fn purchase_reconciliation_service(
        &self,
    ) -> impl abt_core::purchase::reconciliation::PurchaseReconciliationService {
        abt_core::purchase::reconciliation::new_purchase_reconciliation_service(self.pool.clone())
    }

    pub fn payment_request_service(
        &self,
    ) -> impl abt_core::purchase::payment::PaymentRequestService {
        abt_core::purchase::payment::new_payment_request_service(self.pool.clone())
    }

    pub fn misc_request_service(
        &self,
    ) -> impl abt_core::purchase::misc_request::MiscellaneousRequestService {
        abt_core::purchase::misc_request::new_misc_request_service(self.pool.clone())
    }
}
