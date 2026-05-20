//! 服务接口层
//!
//! 定义业务服务的 trait 接口。

mod auth_service;
mod bom_category_service;
mod bom_service;
mod department_service;
mod document_sequence_service;
mod inventory_service;
mod labor_process_service;
mod labor_process_dict_service;
mod location_service;
mod permission_service;
mod excel_service;
mod product_price_service;
mod product_service;
mod quotation_service;
mod role_service;
mod sales_order_service;
mod routing_service;
mod term_service;
mod user_service;
mod warehouse_service;
mod inventory_cascade_service;
mod notification_service;
mod product_watcher_service;
mod scheduled_task_service;
mod workflow_service;

pub use auth_service::AuthService;
pub use bom_category_service::BomCategoryService;
pub use bom_service::{AttributeOverrides, BomService};
pub use department_service::DepartmentService;
pub use document_sequence_service::DocumentSequenceService;
pub use labor_process_service::LaborProcessService;
pub use labor_process_dict_service::LaborProcessDictService;
pub use inventory_service::{InventoryLog, InventoryService};
pub use location_service::LocationService;
pub use permission_service::PermissionService;
pub use excel_service::{ExcelExportService, ExcelImportService, ExcelProgress, ExportRequest, ImportResult, ImportSource, RowError};
pub use product_price_service::{
    AllPriceHistoryQuery, PriceHistoryQuery, PriceLogEntry, PriceLogWithProduct,
    ProductPriceService,
};
pub use product_service::ProductService;
pub use quotation_service::QuotationService;
pub use role_service::RoleService;
pub use sales_order_service::SalesOrderService;
pub use routing_service::RoutingService;
pub use term_service::TermService;
pub use user_service::UserService;
pub use warehouse_service::WarehouseService;
pub use inventory_cascade_service::InventoryCascadeService;
pub use notification_service::NotificationService;
pub use product_watcher_service::ProductWatcherService;
pub use scheduled_task_service::{ScheduledTask, TaskRunResult, TaskStatus};
pub use workflow_service::WorkflowService;

// Re-export executor type from repositories
pub use crate::repositories::Executor;
