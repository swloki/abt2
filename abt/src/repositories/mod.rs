//! 数据访问层
//!
//! 提供 PostgreSQL 数据库访问功能。

mod bom_repo;
mod inventory_repo;
mod labor_process_repo;
mod location_repo;
mod permission_repo;
mod product_price_repo;
mod product_repo;
mod role_repo;
mod term_repo;
mod user_repo;
mod warehouse_repo;

pub use bom_repo::{BomReference, BomRepo, ProductUsageResult};
pub use inventory_repo::InventoryRepo;
pub use labor_process_repo::LaborProcessRepo;
pub use location_repo::LocationRepo;
pub use permission_repo::PermissionRepo;
pub use product_price_repo::ProductPriceRepo;
pub use product_repo::ProductRepo;
pub use role_repo::RoleRepo;
pub use term_repo::TermRepo;
pub use user_repo::UserRepo;
pub use warehouse_repo::WarehouseRepo;

// Re-export Executor from common
pub use common::PgExecutor as Executor;

/// 分页参数
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub page: u32,
    pub page_size: u32,
}

#[allow(dead_code)]
impl PaginationParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }
}

/// 分页结果
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[allow(dead_code)]
impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: &PaginationParams) -> Self {
        let total_pages = ((total as f64) / (pagination.page_size as f64)).ceil() as u32;
        Self {
            items,
            total,
            page: pagination.page,
            page_size: pagination.page_size,
            total_pages,
        }
    }
}
