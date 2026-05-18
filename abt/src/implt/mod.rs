//! 服务实现层
//!
//! 提供服务接口的具体实现。

mod auth_service_impl;
mod bom_service_impl;
pub mod excel;
mod bom_category_impl;
mod department_service_impl;
mod inventory_service_impl;
mod labor_process_service_impl;
mod labor_process_dict_service_impl;
mod location_service_impl;
mod permission_service_impl;
mod product_price_service_impl;
mod product_service_impl;
mod role_service_impl;
mod routing_service_impl;
mod term_service_impl;
mod user_service_impl;
mod warehouse_service_impl;
mod inventory_cascade_service_impl;
mod notification_service_impl;
mod product_watcher_service_impl;
mod stock_alert_task;
mod task_scheduler;
pub mod graph_linter;
pub mod workflow_actions;
pub mod workflow_engine;
pub mod workflow_hooks;
pub mod workflow_worker;

pub use auth_service_impl::AuthServiceImpl;
pub use bom_service_impl::BomServiceImpl;
pub use bom_category_impl::BomCategoryServiceImpl;
pub use department_service_impl::DepartmentServiceImpl;
pub use inventory_service_impl::InventoryServiceImpl;
pub use labor_process_service_impl::LaborProcessServiceImpl;
pub use labor_process_dict_service_impl::LaborProcessDictServiceImpl;
pub use location_service_impl::LocationServiceImpl;
pub use permission_service_impl::PermissionServiceImpl;
pub use product_price_service_impl::ProductPriceServiceImpl;
pub use product_service_impl::ProductServiceImpl;
pub use role_service_impl::RoleServiceImpl;
pub use routing_service_impl::RoutingServiceImpl;
pub use term_service_impl::TermServiceImpl;
pub use user_service_impl::UserServiceImpl;
pub use warehouse_service_impl::WarehouseServiceImpl;
pub use inventory_cascade_service_impl::InventoryCascadeServiceImpl;
pub use notification_service_impl::NotificationServiceImpl;
pub use product_watcher_service_impl::ProductWatcherServiceImpl;
pub use stock_alert_task::StockAlertTask;
pub use task_scheduler::TaskScheduler;
