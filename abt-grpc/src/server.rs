//! gRPC Server 配置和启动

use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

// Re-export config types
pub use crate::config::{get_config, Config};

// Global application state
static APP_STATE: OnceCell<Arc<AppState>> = OnceCell::const_new();

// Global permission cache (abt-core)
static PERMISSION_CACHE: std::sync::OnceLock<std::sync::Arc<abt_core::shared::identity::RolePermissionCache>> =
    std::sync::OnceLock::new();

/// Get the global permission cache
pub fn get_permission_cache() -> &'static std::sync::Arc<abt_core::shared::identity::RolePermissionCache> {
    PERMISSION_CACHE.get().expect("PermissionCache not initialized")
}

pub struct AppState {
    abt_core_pool: sqlx::PgPool,
    workflow_engine: abt_core::workflow::WorkflowEngine,
    shutdown: Arc<AtomicBool>,
    worker_cancel: tokio_util::sync::CancellationToken,
    event_handler_registry: Arc<dyn abt_core::shared::event_bus::EventHandlerRegistry>,
    #[allow(dead_code)]
    event_processor: Arc<abt_core::shared::event_bus::EventProcessor>,
}

impl AppState {
    /// Initialize the global application state using TOML config
    pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
        let config = get_config();

        // Initialize abt-core database pool (abt_v2)
        let abt_core_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connection)
            .connect(&config.database_url)
            .await?;

        // Initialize permission cache (abt-core)
        {
            let cache = abt_core::shared::identity::RolePermissionCache::new(Arc::new(abt_core_pool.clone()));
            cache.load(&abt_core_pool).await?;
            PERMISSION_CACHE.set(Arc::new(cache)).map_err(|_| "PermissionCache already initialized")?;
        }

        // Initialize H3Yun event-driven sync via EventProcessor
        let shutdown = Arc::new(AtomicBool::new(false));
        let h3yun_client = abt_core::h3yun::H3YunClient::new();
        let core_pool = Arc::new(abt_core_pool.clone());

        let registry: Arc<dyn abt_core::shared::event_bus::EventHandlerRegistry> = {
            use abt_core::shared::event_bus::{EventHandlerRegistryImpl, EventHandlerRegistry};
            use abt_core::shared::enums::event::DomainEventType;
            use abt_core::h3yun::{ProductSyncHandler, ProductDeleteHandler, InventorySyncHandler};

            let reg = Arc::new(EventHandlerRegistryImpl::new());
            reg.register(DomainEventType::ProductCreated, Arc::new(ProductSyncHandler::new(core_pool.clone(), h3yun_client.clone())));
            reg.register(DomainEventType::ProductUpdated, Arc::new(ProductSyncHandler::new(core_pool.clone(), h3yun_client.clone())));
            reg.register(DomainEventType::ProductDeleted, Arc::new(ProductDeleteHandler::new(core_pool.clone(), h3yun_client.clone())));
            reg.register(DomainEventType::H3YunInventorySync, Arc::new(InventorySyncHandler::new(core_pool.clone(), h3yun_client)));
            reg
        };

        let event_processor = {
            use abt_core::shared::event_bus::{EventProcessor, DeadLetterServiceImpl, dead_letter::DeadLetterService};
            let dead_letter: Arc<dyn DeadLetterService> = Arc::new(DeadLetterServiceImpl::new());
            let processor = Arc::new(EventProcessor::new(core_pool.clone(), registry.clone(), dead_letter, 3));
            processor.start();
            processor
        };

        // 启动 Workflow Worker（超时扫描/提醒）
        let worker_cancel = tokio_util::sync::CancellationToken::new();
        let worker = abt_core::workflow::worker::WorkflowWorker::new(
            core_pool.clone(),
            worker_cancel.clone(),
        );
        tokio::spawn(async move {
            worker.run().await;
        });

        let workflow_engine = abt_core::workflow::WorkflowEngine::new(core_pool.clone());

        let state = Arc::new(AppState {
            abt_core_pool,
            workflow_engine,
            shutdown: shutdown.clone(),
            worker_cancel,
            event_handler_registry: registry,
            event_processor,
        });

        APP_STATE
            .set(state)
            .map_err(|_| "AppState already initialized")?;

        Ok(())
    }

    /// Get the global application state
    pub async fn get() -> Arc<AppState> {
        APP_STATE.get().expect("AppState not initialized").clone()
    }

    pub fn product_service(&self) -> impl abt_core::master_data::product::ProductService {
        use abt_core::master_data::product::repo::ProductRepo;
        use abt_core::master_data::product::implt::ProductServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: std::sync::Arc<dyn DocumentSequenceService> = std::sync::Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: std::sync::Arc<dyn DomainEventBus> = std::sync::Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: std::sync::Arc<dyn StateMachineService> = std::sync::Arc::new(StateMachineServiceImpl::new(pool, event_bus.clone()));
        ProductServiceImpl::new(ProductRepo, doc_seq, audit, event_bus, state_machine)
    }

    pub fn bom_query_service(&self) -> impl abt_core::master_data::bom::service::BomQueryService {
        use abt_core::master_data::bom::implt::BomQueryServiceImpl;
        use abt_core::master_data::bom::repo::{BomNodeRepo, BomRepo, BomSnapshotRepo};
        BomQueryServiceImpl::new(BomRepo, BomNodeRepo, BomSnapshotRepo)
    }

    pub fn bom_command_service(&self) -> impl abt_core::master_data::bom::service::BomCommandService {
        use abt_core::master_data::bom::implt::BomCommandServiceImpl;
        use abt_core::master_data::bom::repo::{BomNodeRepo, BomRepo, BomSnapshotRepo};
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: std::sync::Arc<dyn DocumentSequenceService> = std::sync::Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: std::sync::Arc<dyn DomainEventBus> = std::sync::Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: std::sync::Arc<dyn StateMachineService> = std::sync::Arc::new(StateMachineServiceImpl::new(pool, event_bus.clone()));
        BomCommandServiceImpl::new(BomRepo, BomNodeRepo, BomSnapshotRepo, doc_seq, audit, event_bus, state_machine)
    }

    pub fn bom_node_service(&self) -> impl abt_core::master_data::bom::service::BomNodeService {
        use abt_core::master_data::bom::implt::BomNodeServiceImpl;
        use abt_core::master_data::bom::repo::{BomNodeRepo, BomRepo};
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let event_bus: std::sync::Arc<dyn DomainEventBus> = std::sync::Arc::new(DomainEventBusImpl::new(pool));
        BomNodeServiceImpl::new(BomRepo, BomNodeRepo, audit, event_bus)
    }

    pub fn bom_cost_service(&self) -> impl abt_core::master_data::bom::service::BomCostService {
        use abt_core::master_data::bom::implt::BomCostServiceImpl;
        use abt_core::master_data::bom::repo::{BomNodeRepo, BomRepo};
        use abt_core::master_data::price::repo::PriceRepo;
        BomCostServiceImpl::new(BomRepo, BomNodeRepo, PriceRepo)
    }

    pub fn warehouse_service(&self) -> impl abt_core::wms::warehouse::WarehouseService {
        use abt_core::wms::warehouse::implt::WarehouseServiceImpl;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        WarehouseServiceImpl::new(pool)
    }

    pub fn inventory_service(&self) -> impl abt_core::wms::inventory::InventoryService {
        use abt_core::wms::inventory::implt::InventoryServiceImpl;
        InventoryServiceImpl::new()
    }

    pub fn price_service(&self) -> impl abt_core::master_data::price::ProductPriceService {
        use abt_core::master_data::price::repo::PriceRepo;
        use abt_core::master_data::price::implt::PriceServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool));
        PriceServiceImpl::new(PriceRepo, audit)
    }

    pub fn labor_process_service(&self) -> impl abt_core::master_data::bom_labor_process::BomLaborProcessService {
        use abt_core::master_data::bom_labor_process::repo::BomLaborProcessRepo;
        use abt_core::master_data::bom_labor_process::implt::BomLaborProcessServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool));
        BomLaborProcessServiceImpl::new(BomLaborProcessRepo, audit)
    }

    pub fn labor_process_dict_service(&self) -> impl abt_core::master_data::labor_process_dict::LaborProcessDictService {
        use abt_core::master_data::labor_process_dict::repo::LaborProcessDictRepo;
        use abt_core::master_data::labor_process_dict::implt::LaborProcessDictServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: std::sync::Arc<dyn DocumentSequenceService> = std::sync::Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: std::sync::Arc<dyn DomainEventBus> = std::sync::Arc::new(DomainEventBusImpl::new(pool));
        LaborProcessDictServiceImpl::new(LaborProcessDictRepo, doc_seq, audit, event_bus)
    }

    pub fn routing_service(&self) -> impl abt_core::master_data::routing::RoutingService {
        use abt_core::master_data::routing::repo::RoutingRepo;
        use abt_core::master_data::routing::implt::RoutingServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let event_bus: std::sync::Arc<dyn DomainEventBus> = std::sync::Arc::new(DomainEventBusImpl::new(pool));
        RoutingServiceImpl::new(RoutingRepo, audit, event_bus)
    }

    pub fn user_service(&self) -> impl abt_core::shared::identity::UserService {
        use abt_core::shared::identity::implt::UserServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit: Arc<dyn AuditLogService> = Arc::new(AuditLogServiceImpl::new(pool.clone()));
        UserServiceImpl::new(pool, audit)
    }

    pub fn role_service(&self) -> impl abt_core::shared::identity::RoleService {
        use abt_core::shared::identity::implt::RoleServiceImpl;
        let pool = Arc::new(self.abt_core_pool.clone());
        let cache = Arc::clone(get_permission_cache());
        RoleServiceImpl::new(pool, cache)
    }

    pub fn permission_service(&self) -> impl abt_core::shared::identity::PermissionService {
        use abt_core::shared::identity::implt::PermissionServiceImpl;
        let cache = Arc::clone(get_permission_cache());
        PermissionServiceImpl::new(cache)
    }

    pub fn department_service(&self) -> impl abt_core::shared::identity::department_service::DepartmentService {
        use abt_core::shared::identity::implt::DepartmentServiceImpl;
        let pool = Arc::new(self.abt_core_pool.clone());
        DepartmentServiceImpl::new(pool)
    }

    pub fn bom_category_service(&self) -> impl abt_core::master_data::bom::service::BomCategoryService {
        use abt_core::master_data::bom::repo::BomCategoryRepo;
        use abt_core::master_data::bom::implt::BomCategoryServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        let pool = std::sync::Arc::new(self.abt_core_pool.clone());
        let audit: std::sync::Arc<dyn AuditLogService> = std::sync::Arc::new(AuditLogServiceImpl::new(pool));
        BomCategoryServiceImpl::new(BomCategoryRepo, audit)
    }

    pub fn category_service(&self) -> impl abt_core::master_data::category::CategoryService {
        use abt_core::master_data::category::repo::CategoryRepo;
        use abt_core::master_data::category::implt::CategoryServiceImpl;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit = Arc::new(AuditLogServiceImpl::new(pool)) as Arc<dyn abt_core::shared::audit_log::service::AuditLogService>;
        CategoryServiceImpl::new(CategoryRepo, audit)
    }

    pub async fn begin_core_transaction(&self) -> anyhow::Result<sqlx::Transaction<'static, sqlx::Postgres>> {
        self.abt_core_pool.begin().await.map_err(Into::into)
    }

    pub fn inventory_cascade_service(&self) -> impl abt_core::wms::inventory_cascade::InventoryCascadeService {
        use abt_core::wms::inventory_cascade::implt::InventoryCascadeServiceImpl;
        InventoryCascadeServiceImpl::new()
    }

    pub fn notification_service(&self) -> impl abt_core::shared::notification::NotificationService {
        use abt_core::shared::notification::implt::NotificationServiceImpl;
        use abt_core::shared::notification::repo::NotificationRepo;
        NotificationServiceImpl::new(NotificationRepo)
    }

    pub fn product_watcher_service(&self) -> impl abt_core::master_data::product_watcher::ProductWatcherService {
        use abt_core::master_data::product_watcher::implt::ProductWatcherServiceImpl;
        ProductWatcherServiceImpl::new()
    }

    /// abt-core QuotationService
    pub fn quotation_core_service(&self) -> impl abt_core::sales::quotation::QuotationService {
        use abt_core::sales::quotation::implt::QuotationServiceImpl;
        use abt_core::sales::quotation::repo::{QuotationItemRepo, QuotationRepo};
        use abt_core::master_data::customer::implt::CustomerServiceImpl;
        use abt_core::master_data::customer::repo::{CustomerRepo, CustomerContactRepo, CustomerAddressRepo};
        use abt_core::master_data::customer::service::CustomerService;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit: Arc<dyn AuditLogService> = Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: Arc<dyn DocumentSequenceService> = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: Arc<dyn DomainEventBus> = Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: Arc<dyn StateMachineService> = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
        let customer_svc: Arc<dyn CustomerService> = Arc::new(CustomerServiceImpl::new(
            CustomerRepo,
            CustomerContactRepo,
            CustomerAddressRepo,
            doc_seq.clone(),
            audit.clone(),
            event_bus.clone(),
            state_machine.clone(),
        ));
        QuotationServiceImpl::new(
            QuotationRepo,
            QuotationItemRepo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            customer_svc,
        )
    }

    /// abt-core SalesOrderService
    pub fn sales_order_core_service(&self) -> impl abt_core::sales::sales_order::SalesOrderService {
        use abt_core::sales::sales_order::implt::SalesOrderServiceImpl;
        use abt_core::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
        use abt_core::sales::quotation::implt::QuotationServiceImpl;
        use abt_core::sales::quotation::repo::{QuotationItemRepo, QuotationRepo};
        use abt_core::sales::quotation::QuotationService;
        use abt_core::master_data::customer::implt::CustomerServiceImpl;
        use abt_core::master_data::customer::repo::{CustomerRepo, CustomerContactRepo, CustomerAddressRepo};
        use abt_core::master_data::customer::service::CustomerService;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::document_link::implt::DocumentLinkServiceImpl;
        use abt_core::shared::document_link::service::DocumentLinkService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::inventory_reservation::implt::InventoryReservationServiceImpl;
        use abt_core::shared::inventory_reservation::service::InventoryReservationService;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit: Arc<dyn AuditLogService> = Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: Arc<dyn DocumentSequenceService> = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: Arc<dyn DomainEventBus> = Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: Arc<dyn StateMachineService> = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
        let customer_svc: Arc<dyn CustomerService> = Arc::new(CustomerServiceImpl::new(
            CustomerRepo,
            CustomerContactRepo,
            CustomerAddressRepo,
            doc_seq.clone(),
            audit.clone(),
            event_bus.clone(),
            state_machine.clone(),
        ));
        let quotation_svc: Arc<dyn QuotationService> = Arc::new(QuotationServiceImpl::new(
            QuotationRepo,
            QuotationItemRepo,
            doc_seq.clone(),
            state_machine.clone(),
            audit.clone(),
            event_bus.clone(),
            customer_svc.clone(),
        ));
        let doc_link: Arc<dyn DocumentLinkService> = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
        let inv_res: Arc<dyn InventoryReservationService> = Arc::new(InventoryReservationServiceImpl::new(pool));
        SalesOrderServiceImpl::new(
            SalesOrderRepo,
            SalesOrderItemRepo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            customer_svc,
            quotation_svc,
            doc_link,
            inv_res,
        )
    }

    /// abt-core SalesReturnService
    pub fn sales_return_core_service(&self) -> impl abt_core::sales::sales_return::SalesReturnService {
        use abt_core::sales::sales_return::implt::SalesReturnServiceImpl;
        use abt_core::sales::sales_return::repo::{SalesReturnItemRepo, SalesReturnRepo};
        use abt_core::sales::sales_order::repo::SalesOrderItemRepo;
        use abt_core::sales::shipping_request::implt::ShippingRequestServiceImpl;
        use abt_core::sales::shipping_request::repo::{ShippingRequestItemRepo, ShippingRequestRepo};
        use abt_core::sales::shipping_request::service::ShippingRequestService;
        use abt_core::sales::sales_order::repo::{SalesOrderItemRepo as SOItemRepo, SalesOrderRepo};
        use abt_core::sales::sales_order::SalesOrderService;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::cost_entry::implt::CostEntryServiceImpl;
        use abt_core::shared::cost_entry::service::CostEntryService;
        use abt_core::shared::document_link::implt::DocumentLinkServiceImpl;
        use abt_core::shared::document_link::service::DocumentLinkService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        use abt_core::shared::inventory_reservation::implt::InventoryReservationServiceImpl;
        use abt_core::shared::inventory_reservation::service::InventoryReservationService;
        use abt_core::qms::rma::implt::RmaServiceImpl;
        use abt_core::qms::rma::service::RmaService;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit: Arc<dyn AuditLogService> = Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: Arc<dyn DocumentSequenceService> = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: Arc<dyn DomainEventBus> = Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: Arc<dyn StateMachineService> = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
        let doc_link: Arc<dyn DocumentLinkService> = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
        let cost_entry: Arc<dyn CostEntryService> = Arc::new(CostEntryServiceImpl::new(pool.clone()));
        let inv_res: Arc<dyn InventoryReservationService> = Arc::new(InventoryReservationServiceImpl::new(pool.clone()));
        // ShippingRequestService (dependency of SalesReturnService)
        // Build SalesOrderServiceImpl inline since impl Trait cannot be converted to Arc<dyn Trait>
        let sales_order_svc: Arc<dyn SalesOrderService> = {
            use abt_core::sales::sales_order::implt::SalesOrderServiceImpl;
            use abt_core::sales::sales_order::repo::{SalesOrderItemRepo as SOIR, SalesOrderRepo as SOR};
            use abt_core::sales::quotation::implt::QuotationServiceImpl as QSI;
            use abt_core::sales::quotation::repo::{QuotationItemRepo as QIIR, QuotationRepo as QR};
            use abt_core::sales::quotation::QuotationService as QS;
            use abt_core::master_data::customer::implt::CustomerServiceImpl as CSI;
            use abt_core::master_data::customer::repo::{CustomerRepo as CR, CustomerContactRepo as CCR, CustomerAddressRepo as CAR};
            use abt_core::master_data::customer::service::CustomerService as CS;
            let customer_svc_inner: Arc<dyn CS> = Arc::new(CSI::new(
                CR, CCR, CAR,
                doc_seq.clone(), audit.clone(), event_bus.clone(), state_machine.clone(),
            ));
            let quotation_svc_inner: Arc<dyn QS> = Arc::new(QSI::new(
                QR, QIIR,
                doc_seq.clone(), state_machine.clone(), audit.clone(), event_bus.clone(), customer_svc_inner.clone(),
            ));
            let doc_link_inner: Arc<dyn abt_core::shared::document_link::service::DocumentLinkService> = Arc::new(abt_core::shared::document_link::implt::DocumentLinkServiceImpl::new(pool.clone()));
            let inv_res_inner: Arc<dyn abt_core::shared::inventory_reservation::service::InventoryReservationService> = Arc::new(abt_core::shared::inventory_reservation::implt::InventoryReservationServiceImpl::new(pool.clone()));
            Arc::new(SalesOrderServiceImpl::new(
                SOR, SOIR,
                doc_seq.clone(), state_machine.clone(), audit.clone(), event_bus.clone(),
                customer_svc_inner, quotation_svc_inner, doc_link_inner, inv_res_inner,
            ))
        };
        let shipping_svc: Arc<dyn ShippingRequestService> = Arc::new(ShippingRequestServiceImpl::new(
            ShippingRequestRepo,
            ShippingRequestItemRepo,
            SalesOrderRepo,
            SOItemRepo,
            doc_seq.clone(),
            state_machine.clone(),
            audit.clone(),
            event_bus.clone(),
            sales_order_svc,
            doc_link.clone(),
            inv_res,
            cost_entry.clone(),
            // QMS: use a simplified approach - create a basic inspection result service
            {
                use abt_core::qms::inspection_result::implt::InspectionResultServiceImpl;
                use abt_core::qms::inspection_result::service::InspectionResultService;
                use abt_core::qms::inspection_specification::implt::InspectionSpecificationServiceImpl;
                use abt_core::qms::inspection_specification::service::InspectionSpecificationService;
                use abt_core::shared::idempotency::implt::IdempotencyServiceImpl;
                use abt_core::shared::idempotency::service::IdempotencyService;
                let idempotency: Arc<dyn IdempotencyService> = Arc::new(IdempotencyServiceImpl::new(pool.clone()));
                let spec_svc: Arc<dyn InspectionSpecificationService> = Arc::new(InspectionSpecificationServiceImpl::new(
                    pool.clone(),
                    doc_seq.clone(),
                    state_machine.clone(),
                    audit.clone(),
                ));
                let qms: Arc<dyn InspectionResultService> = Arc::new(InspectionResultServiceImpl::new(
                    pool.clone(),
                    doc_seq.clone(),
                    state_machine.clone(),
                    event_bus.clone(),
                    audit.clone(),
                    idempotency,
                    spec_svc,
                ));
                qms
            },
        ));
        // RmaService (dependency of SalesReturnService)
        let rma: Arc<dyn RmaService> = Arc::new(RmaServiceImpl::new(
            pool,
            doc_seq.clone(),
            state_machine.clone(),
            event_bus.clone(),
            audit.clone(),
            doc_link.clone(),
        ));
        SalesReturnServiceImpl::new(
            SalesReturnRepo,
            SalesReturnItemRepo,
            SalesOrderItemRepo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            shipping_svc,
            doc_link,
            cost_entry,
            rma,
        )
        // Note: rma takes ownership of last clone of these arcs, consuming them
    }

    /// abt-core ShippingRequestService
    pub fn shipping_request_core_service(&self) -> impl abt_core::sales::shipping_request::ShippingRequestService {
        use abt_core::sales::shipping_request::implt::ShippingRequestServiceImpl;
        use abt_core::sales::shipping_request::repo::{ShippingRequestItemRepo, ShippingRequestRepo};
        use abt_core::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
        use abt_core::sales::sales_order::SalesOrderService;
        use abt_core::shared::audit_log::implt::AuditLogServiceImpl;
        use abt_core::shared::audit_log::service::AuditLogService;
        use abt_core::shared::cost_entry::implt::CostEntryServiceImpl;
        use abt_core::shared::cost_entry::service::CostEntryService;
        use abt_core::shared::document_link::implt::DocumentLinkServiceImpl;
        use abt_core::shared::document_link::service::DocumentLinkService;
        use abt_core::shared::document_sequence::implt::DocumentSequenceServiceImpl;
        use abt_core::shared::document_sequence::service::DocumentSequenceService;
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        use abt_core::shared::event_bus::service::DomainEventBus;
        use abt_core::shared::inventory_reservation::implt::InventoryReservationServiceImpl;
        use abt_core::shared::inventory_reservation::service::InventoryReservationService;
        use abt_core::shared::state_machine::implt::StateMachineServiceImpl;
        use abt_core::shared::state_machine::service::StateMachineService;
        use abt_core::qms::inspection_result::implt::InspectionResultServiceImpl;
        use abt_core::qms::inspection_result::service::InspectionResultService;
        use abt_core::qms::inspection_specification::implt::InspectionSpecificationServiceImpl;
        use abt_core::qms::inspection_specification::service::InspectionSpecificationService;
        use abt_core::shared::idempotency::implt::IdempotencyServiceImpl;
        use abt_core::shared::idempotency::service::IdempotencyService;
        let pool = Arc::new(self.abt_core_pool.clone());
        let audit: Arc<dyn AuditLogService> = Arc::new(AuditLogServiceImpl::new(pool.clone()));
        let doc_seq: Arc<dyn DocumentSequenceService> = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
        let event_bus: Arc<dyn DomainEventBus> = Arc::new(DomainEventBusImpl::new(pool.clone()));
        let state_machine: Arc<dyn StateMachineService> = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
        let doc_link: Arc<dyn DocumentLinkService> = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
        let cost_entry: Arc<dyn CostEntryService> = Arc::new(CostEntryServiceImpl::new(pool.clone()));
        let inv_res: Arc<dyn InventoryReservationService> = Arc::new(InventoryReservationServiceImpl::new(pool.clone()));
        let sales_order_svc: Arc<dyn SalesOrderService> = {
            // Build inline since impl Trait cannot be converted to Arc<dyn Trait>
            use abt_core::sales::sales_order::implt::SalesOrderServiceImpl;
            use abt_core::sales::sales_order::repo::{SalesOrderItemRepo as SOIR, SalesOrderRepo as SOR};
            use abt_core::sales::quotation::implt::QuotationServiceImpl as QSI;
            use abt_core::sales::quotation::repo::{QuotationItemRepo as QIIR, QuotationRepo as QR};
            use abt_core::sales::quotation::QuotationService as QS;
            use abt_core::master_data::customer::implt::CustomerServiceImpl as CSI;
            use abt_core::master_data::customer::repo::{CustomerRepo as CR, CustomerContactRepo as CCR, CustomerAddressRepo as CAR};
            use abt_core::master_data::customer::service::CustomerService as CS;
            let customer_svc_inner: Arc<dyn CS> = Arc::new(CSI::new(
                CR, CCR, CAR,
                doc_seq.clone(), audit.clone(), event_bus.clone(), state_machine.clone(),
            ));
            let quotation_svc_inner: Arc<dyn QS> = Arc::new(QSI::new(
                QR, QIIR,
                doc_seq.clone(), state_machine.clone(), audit.clone(), event_bus.clone(), customer_svc_inner.clone(),
            ));
            let doc_link_inner: Arc<dyn abt_core::shared::document_link::service::DocumentLinkService> = Arc::new(abt_core::shared::document_link::implt::DocumentLinkServiceImpl::new(pool.clone()));
            let inv_res_inner: Arc<dyn abt_core::shared::inventory_reservation::service::InventoryReservationService> = Arc::new(abt_core::shared::inventory_reservation::implt::InventoryReservationServiceImpl::new(pool.clone()));
            Arc::new(SalesOrderServiceImpl::new(
                SOR, SOIR,
                doc_seq.clone(), state_machine.clone(), audit.clone(), event_bus.clone(),
                customer_svc_inner, quotation_svc_inner, doc_link_inner, inv_res_inner,
            ))
        };
        let idempotency: Arc<dyn IdempotencyService> = Arc::new(IdempotencyServiceImpl::new(pool.clone()));
        let spec_svc: Arc<dyn InspectionSpecificationService> = Arc::new(InspectionSpecificationServiceImpl::new(
            pool.clone(),
            doc_seq.clone(),
            state_machine.clone(),
            audit.clone(),
        ));
        let qms: Arc<dyn InspectionResultService> = Arc::new(InspectionResultServiceImpl::new(
            pool.clone(),
            doc_seq.clone(),
            state_machine.clone(),
            event_bus.clone(),
            audit.clone(),
            idempotency,
            spec_svc,
        ));
        ShippingRequestServiceImpl::new(
            ShippingRequestRepo,
            ShippingRequestItemRepo,
            SalesOrderRepo,
            SalesOrderItemRepo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            sales_order_svc,
            doc_link,
            inv_res,
            cost_entry,
            qms,
        )
    }

    pub fn workflow_service(&self) -> abt_core::workflow::WorkflowEngine {
        self.workflow_engine.clone()
    }

    pub fn event_bus(&self) -> Arc<dyn abt_core::shared::event_bus::DomainEventBus> {
        use abt_core::shared::event_bus::implt::DomainEventBusImpl;
        Arc::new(DomainEventBusImpl::new(Arc::new(self.abt_core_pool.clone())))
    }

    pub fn event_handler_registry(&self) -> Arc<dyn abt_core::shared::event_bus::EventHandlerRegistry> {
        self.event_handler_registry.clone()
    }

    pub fn auth_service(&self) -> impl abt_core::shared::identity::AuthService {
        use abt_core::shared::identity::implt::AuthServiceImpl;
        let pool = Arc::new(self.abt_core_pool.clone());
        let config = get_config();
        AuthServiceImpl::new(pool, config.jwt_secret.clone())
    }

    pub fn core_pool(&self) -> sqlx::PgPool {
        self.abt_core_pool.clone()
    }
}

pub async fn start_server(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    AppState::init().await?;
    let state = AppState::get().await;
    let shutdown = state.shutdown.clone();

    let reflection_service = Builder::configure()
        .build_v1()
        .expect("Failed to build reflection service");

    use crate::handlers::{
        AbtBomServiceServer, AbtExcelServiceServer, AbtInventoryServiceServer,
        AbtLaborProcessServiceServer, AbtLaborProcessDictServiceServer, AbtLocationServiceServer, AbtPriceServiceServer,
        AbtProductServiceServer, AbtRoutingServiceServer, AbtSyncServiceServer, AbtTermServiceServer, AbtWarehouseServiceServer,
        AbtWorkflowServiceServer,
        QuotationServiceServer, SalesOrderServiceServer, SalesReturnServiceServer, ShippingRequestServiceServer,
        AuthServiceServer, AbtBomCategoryServiceServer, AbtCategoryServiceServer, DepartmentServiceServer,
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
        .add_service(AbtCategoryServiceServer::with_interceptor(
            crate::handlers::category::CategoryHandler::new(), auth_interceptor,
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
        .add_service(crate::handlers::AbtNotificationServiceServer::with_interceptor(
            crate::handlers::notification::NotificationHandler::new(), auth_interceptor,
        ))
        .add_service(AbtSyncServiceServer::with_interceptor(
            crate::handlers::sync_handler::SyncHandler::new(), auth_interceptor,
        ))
        .add_service(AbtWorkflowServiceServer::with_interceptor(
            crate::handlers::workflow::WorkflowHandler::new(), auth_interceptor,
        ))
        .add_service(QuotationServiceServer::with_interceptor(
            crate::handlers::quotation::QuotationHandler::new(), auth_interceptor,
        ))
        .add_service(SalesOrderServiceServer::with_interceptor(
            crate::handlers::sales_order::SalesOrderHandler::new(), auth_interceptor,
        ))
        .add_service(SalesReturnServiceServer::with_interceptor(
            crate::handlers::sales_return::SalesReturnHandler::new(), auth_interceptor,
        ))
        .add_service(ShippingRequestServiceServer::with_interceptor(
            crate::handlers::shipping_request::ShippingRequestHandler::new(), auth_interceptor,
        ))
        .serve_with_shutdown(addr, async move {
            tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
            tracing::info!("Shutdown signal received, stopping background tasks...");
            state.worker_cancel.cancel();
            shutdown.store(true, std::sync::atomic::Ordering::Release);
        })
        .await?;

    tracing::info!("Server stopped.");
    Ok(())
}
