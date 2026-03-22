//! 服务实现层
//!
//! 提供服务接口的具体实现。

mod bom_service_impl;
mod inventory_service_impl;
mod labor_process_service_impl;
mod location_service_impl;
mod permission_service_impl;
mod product_excel_service_impl;
mod product_price_service_impl;
mod product_service_impl;
mod role_service_impl;
mod term_service_impl;
mod user_service_impl;
mod warehouse_service_impl;

pub use bom_service_impl::BomServiceImpl;
pub use inventory_service_impl::InventoryServiceImpl;
pub use labor_process_service_impl::LaborProcessServiceImpl;
pub use location_service_impl::LocationServiceImpl;
pub use permission_service_impl::PermissionServiceImpl;
pub use product_excel_service_impl::ProductExcelServiceImpl;
pub use product_price_service_impl::ProductPriceServiceImpl;
pub use product_service_impl::ProductServiceImpl;
pub use role_service_impl::RoleServiceImpl;
pub use term_service_impl::TermServiceImpl;
pub use user_service_impl::UserServiceImpl;
pub use warehouse_service_impl::WarehouseServiceImpl;
