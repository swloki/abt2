//! 服务接口层
//!
//! 定义业务服务的 trait 接口。

mod auth_service;
mod bom_category_service;
mod bom_service;
mod department_service;
mod inventory_service;
mod labor_process_service;
mod location_service;
mod permission_service;
mod product_excel_service;
mod product_price_service;
mod product_service;
mod role_service;
mod term_service;
mod user_service;
mod warehouse_service;

pub use auth_service::AuthService;
pub use bom_category_service::BomCategoryService;
pub use bom_service::BomService;
pub use department_service::DepartmentService;
pub use labor_process_service::LaborProcessService;
pub use inventory_service::{InventoryLog, InventoryService};
pub use location_service::LocationService;
pub use permission_service::PermissionService;
pub use product_excel_service::{ExcelProgress, ImportResult, ProductExcelService};
pub use product_price_service::{
    AllPriceHistoryQuery, PriceHistoryQuery, PriceLogEntry, PriceLogWithProduct,
    ProductPriceService,
};
pub use product_service::ProductService;
pub use role_service::RoleService;
pub use term_service::TermService;
pub use user_service::UserService;
pub use warehouse_service::WarehouseService;

// Re-export executor type from repositories
pub use crate::repositories::Executor;
