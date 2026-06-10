use std::sync::Arc;

use crate::config::Config;
use abt_core::shared::identity::RolePermissionCache;
use abt_core::shared::types::{PgPool, PgPoolOptions};
use abt_core::shared::excel::ImportResult;
use chrono::{DateTime, Utc};
use tower_sessions_file_store::FileSessionStorage;
use dashmap::DashMap;
use std::sync::atomic::AtomicI64;
use tokio::sync::Semaphore;

/// 任务数据保留时长（秒）
const TASK_TTL_SECS: i64 = 1800; // 30 分钟

/// 导入任务状态（内存存储）
#[derive(Debug)]
pub struct ImportTaskState {
    pub status: TaskStatus,
    pub current: usize,
    pub total: usize,
    pub result: Option<ImportResult>,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

/// 导出文件信息（内存存储）
#[derive(Debug, Clone)]
pub struct ExportFileInfo {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub session_store: FileSessionStorage,
    pub permission_cache: Arc<RolePermissionCache>,
    pub import_progress: Arc<DashMap<i64, ImportTaskState>>,
    pub export_files: Arc<DashMap<i64, ExportFileInfo>>,
    next_task_id: Arc<AtomicI64>,
    /// 导入并发信号量（限制同时运行的导入任务数）
    pub import_semaphore: Arc<Semaphore>,
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

        let import_progress: Arc<DashMap<i64, ImportTaskState>> = Arc::new(DashMap::new());
        let export_files: Arc<DashMap<i64, ExportFileInfo>> = Arc::new(DashMap::new());

        // 启动后台清理任务，定期淘汰过期数据
        {
            let import_progress = import_progress.clone();
            let export_files = export_files.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                    let cutoff = Utc::now() - chrono::Duration::seconds(TASK_TTL_SECS);
                    import_progress.retain(|_, v| v.created_at > cutoff);
                    export_files.retain(|_, v| v.created_at > cutoff);
                }
            });
        }

        Ok(Self {
            pool,
            jwt_secret: config.jwt_secret.clone(),
            jwt_expiration_hours: config.jwt_expiration_hours,
            session_store,
            permission_cache,
            import_progress,
            export_files,
            next_task_id: Arc::new(AtomicI64::new(1)),
            import_semaphore: Arc::new(Semaphore::new(5)),
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

    // ── WMS (Inventory Management) Services ──

    pub fn arrival_notice_service(
        &self,
    ) -> impl abt_core::wms::arrival_notice::ArrivalNoticeService {
        abt_core::wms::arrival_notice::new_arrival_notice_service(self.pool.clone())
    }

    pub fn inventory_service(&self) -> impl abt_core::wms::inventory::InventoryService {
        abt_core::wms::inventory::new_inventory_service()
    }

    pub fn inventory_transaction_service(
        &self,
    ) -> impl abt_core::wms::inventory_transaction::InventoryTransactionService {
        abt_core::wms::inventory_transaction::new_inventory_transaction_service(self.pool.clone())
    }

    pub fn material_requisition_service(
        &self,
    ) -> impl abt_core::wms::material_requisition::MaterialRequisitionService {
        abt_core::wms::material_requisition::new_material_requisition_service(self.pool.clone())
    }

    pub fn backflush_service(&self) -> impl abt_core::wms::backflush::BackflushService {
        abt_core::wms::backflush::new_backflush_service(self.pool.clone())
    }

    pub fn cycle_count_service(&self) -> impl abt_core::wms::cycle_count::CycleCountService {
        abt_core::wms::cycle_count::new_cycle_count_service(self.pool.clone())
    }

    pub fn transfer_service(&self) -> impl abt_core::wms::transfer::TransferService {
        abt_core::wms::transfer::new_transfer_service(self.pool.clone())
    }

    pub fn form_conversion_service(
        &self,
    ) -> impl abt_core::wms::form_conversion::FormConversionService {
        abt_core::wms::form_conversion::new_form_conversion_service(self.pool.clone())
    }

    pub fn inventory_lock_service(
        &self,
    ) -> impl abt_core::wms::inventory_lock::InventoryLockService {
        abt_core::wms::inventory_lock::new_inventory_lock_service(self.pool.clone())
    }

    pub fn stock_ledger_service(&self) -> impl abt_core::wms::stock_ledger::StockLedgerService {
        abt_core::wms::stock_ledger::new_stock_ledger_service(self.pool.clone())
    }

    pub fn strategy_service(&self) -> impl abt_core::wms::strategy::StrategyService {
        abt_core::wms::strategy::new_strategy_service(self.pool.clone())
    }

    pub fn inventory_cascade_service(
        &self,
    ) -> impl abt_core::wms::inventory_cascade::InventoryCascadeService {
        abt_core::wms::inventory_cascade::new_inventory_cascade_service()
    }

    pub fn bom_query_service(&self) -> impl abt_core::master_data::bom::BomQueryService {
        abt_core::master_data::bom::new_bom_query_service(self.pool.clone())
    }
    pub fn bom_command_service(&self) -> impl abt_core::master_data::bom::BomCommandService {
        abt_core::master_data::bom::new_bom_command_service(self.pool.clone())
    }

    pub fn bom_node_service(&self) -> impl abt_core::master_data::bom::BomNodeService {
        abt_core::master_data::bom::new_bom_node_service(self.pool.clone())
    }

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

    pub fn department_service(
        &self,
    ) -> impl abt_core::shared::identity::DepartmentService {
        abt_core::shared::identity::implt::DepartmentServiceImpl::new(self.pool.clone())
    }

    pub fn role_service(&self) -> impl abt_core::shared::identity::RoleService {
        abt_core::shared::identity::implt::RoleServiceImpl::new(self.pool.clone(), self.permission_cache.clone())
    }

    // ── Master Data Services ──

    pub fn category_service(&self) -> impl abt_core::master_data::category::CategoryService {
        abt_core::master_data::category::new_category_service(self.pool.clone())
    }

    pub fn product_price_service(
        &self,
    ) -> impl abt_core::master_data::price::ProductPriceService {
        abt_core::master_data::price::new_product_price_service(self.pool.clone())
    }

    pub fn bom_category_service(&self) -> impl abt_core::master_data::bom::BomCategoryService {
        abt_core::master_data::bom::new_bom_category_service(self.pool.clone())
    }

    pub fn labor_process_dict_service(
        &self,
    ) -> impl abt_core::master_data::labor_process_dict::LaborProcessDictService {
        abt_core::master_data::labor_process_dict::new_labor_process_dict_service(
            self.pool.clone(),
        )
    }
    pub fn product_watcher_service(
        &self,
    ) -> impl abt_core::master_data::product_watcher::ProductWatcherService {
        abt_core::master_data::product_watcher::new_product_watcher_service(self.pool.clone())
    }


    pub fn bom_cost_service(&self) -> impl abt_core::master_data::bom::BomCostService {
        abt_core::master_data::bom::new_bom_cost_service(self.pool.clone())
    }

    pub fn bom_labor_process_service(
        &self,
    ) -> impl abt_core::master_data::bom_labor_process::BomLaborProcessService {
        abt_core::master_data::bom_labor_process::new_bom_labor_process_service(self.pool.clone())
    }

    // ── MES (Production) Services ──

    pub fn production_plan_service(
        &self,
    ) -> impl abt_core::mes::production_plan::ProductionPlanService {
        abt_core::mes::production_plan::new_production_plan_service(self.pool.clone())
    }

    pub fn work_order_service(&self) -> impl abt_core::mes::work_order::WorkOrderService {
        abt_core::mes::work_order::new_work_order_service(self.pool.clone())
    }

    pub fn production_batch_service(
        &self,
    ) -> impl abt_core::mes::production_batch::ProductionBatchService {
        abt_core::mes::production_batch::new_production_batch_service(self.pool.clone())
    }

    pub fn work_report_service(&self) -> impl abt_core::mes::work_report::WorkReportService {
        abt_core::mes::work_report::new_work_report_service(self.pool.clone())
    }

    pub fn production_inspection_service(
        &self,
    ) -> impl abt_core::mes::production_inspection::ProductionInspectionService {
        abt_core::mes::production_inspection::new_production_inspection_service(self.pool.clone())
    }

    pub fn production_receipt_service(
        &self,
    ) -> impl abt_core::mes::production_receipt::ProductionReceiptService {
        abt_core::mes::production_receipt::new_production_receipt_service(self.pool.clone())
    }
    pub fn mes_dashboard_service(&self) -> impl abt_core::mes::dashboard::MesDashboardService {
        abt_core::mes::dashboard::new_mes_dashboard_service(self.pool.clone())
    }
    pub fn production_exception_service(&self) -> impl abt_core::mes::production_exception::ProductionExceptionService {
        abt_core::mes::production_exception::new_production_exception_service(self.pool.clone())
    }

    // ── OM (Outsourcing) Services ──

    pub fn outsourcing_order_service(
        &self,
    ) -> impl abt_core::om::outsourcing_order::OutsourcingOrderService {
        abt_core::om::outsourcing_order::new_outsourcing_order_service(self.pool.clone())
    }

    pub fn outsourcing_tracking_service(
        &self,
    ) -> impl abt_core::om::outsourcing_tracking::OutsourcingTrackingService {
        abt_core::om::outsourcing_tracking::new_outsourcing_tracking_service()
    }
    // ── QMS (Quality Management) Services ──
    pub fn inspection_specification_service(
        &self,
    ) -> impl abt_core::qms::inspection_specification::InspectionSpecificationService {
        abt_core::qms::inspection_specification::new_inspection_specification_service(self.pool.clone())
    }
    pub fn inspection_result_service(
        &self,
    ) -> impl abt_core::qms::inspection_result::InspectionResultService {
        abt_core::qms::inspection_result::new_inspection_result_service(self.pool.clone())
    }
    pub fn mrb_service(&self) -> impl abt_core::qms::mrb::MrbService {
        abt_core::qms::mrb::new_mrb_service(self.pool.clone())
    }
    pub fn rma_service(&self) -> impl abt_core::qms::rma::RmaService {
        abt_core::qms::rma::new_rma_service(self.pool.clone())
    }
    // ── FMS (Financial Management) Services ──
    pub fn cash_journal_service(
        &self,
    ) -> impl abt_core::fms::cash_journal::CashJournalService {
        abt_core::fms::cash_journal::new_cash_journal_service(self.pool.clone())
    }
    pub fn expense_service(
        &self,
    ) -> impl abt_core::fms::expense::ExpenseReimbursementService {
        abt_core::fms::expense::new_expense_service(self.pool.clone())
    }
    pub fn write_off_service(&self) -> impl abt_core::fms::write_off::WriteOffService {
        abt_core::fms::write_off::new_write_off_service(self.pool.clone())
    }
    pub fn cost_accounting_service(
        &self,
    ) -> impl abt_core::fms::cost_accounting::CostAccountingService {
        abt_core::fms::cost_accounting::new_cost_accounting_service()
    }

    /// 生成下一个 task_id
    pub fn next_task_id(&self) -> i64 {
        self.next_task_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// 存储导出文件，返回 task_id
    pub fn store_export_file(&self, bytes: Vec<u8>, filename: &str, user_id: i64) -> i64 {
        let id = self.next_task_id();
        self.export_files.insert(id, ExportFileInfo {
            filename: filename.to_string(),
            bytes,
            user_id,
            created_at: Utc::now(),
        });
        id
    }

    /// 获取导出文件（验证 user_id 归属）
    pub fn get_export_file(&self, task_id: i64, user_id: i64) -> Option<ExportFileInfo> {
        self.export_files.get(&task_id)
            .filter(|r| r.value().user_id == user_id)
            .map(|r| r.value().clone())
    }
}
