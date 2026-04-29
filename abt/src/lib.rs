//! ABT - BOM 管理系统核心库
//!
//! 提供 NAPI 绑定，可被 Node.js 直接调用。

#![allow(non_snake_case)]
#![allow(ambiguous_glob_reexports)]

use sqlx::postgres::PgPool;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{Mutex, OnceCell};

// Internal modules
mod implt;
mod permission_cache;
pub mod models;
pub mod repositories;
pub mod service;

pub use permission_cache::RolePermissionCache;

#[cfg(test)]
mod tests;

// Public API (models and service traits)
pub use models::*;
pub use service::*;

// Re-export Excel implementations for gRPC handler use
pub use implt::excel;

// ============================================================================
// App Context
// ============================================================================

/// 应用上下文
///
/// 管理 PostgreSQL 连接池。
pub struct AppContext {
    pool: PgPool,
}

impl AppContext {
    /// 获取数据库连接池引用
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 获取一个新的数据库连接
    pub async fn acquire(&self) -> anyhow::Result<sqlx::pool::PoolConnection<sqlx::Postgres>> {
        Ok(self.pool.acquire().await?)
    }

    /// 开始一个新的事务
    pub async fn begin_transaction(
        &self,
    ) -> anyhow::Result<sqlx::Transaction<'static, sqlx::Postgres>> {
        Ok(self.pool.begin().await?)
    }
}

// ============================================================================
// Global Context Management
// ============================================================================

static CONTEXT: OnceCell<AppContext> = OnceCell::const_new();
static INIT_LOCK: Mutex<()> = Mutex::const_new(());

// Permission cache singleton
static PERMISSION_CACHE: OnceLock<RolePermissionCache> = OnceLock::new();

/// Get the global permission cache
pub fn get_permission_cache() -> &'static RolePermissionCache {
    PERMISSION_CACHE.get_or_init(RolePermissionCache::new)
}

/// 获取全局应用上下文
pub async fn get_context() -> &'static AppContext {
    if let Some(ctx) = CONTEXT.get() {
        return ctx;
    }

    let _guard = INIT_LOCK.lock().await;
    if let Some(ctx) = CONTEXT.get() {
        return ctx;
    }

    panic!("ABT context not initialized. Call init_context_with_pool() first.");
}

/// 使用外部连接池初始化全局应用上下文（用于 gRPC 服务）
pub async fn init_context_with_pool(pool: PgPool) {
    if CONTEXT.get().is_some() {
        return;
    }

    let _guard = INIT_LOCK.lock().await;
    if CONTEXT.get().is_some() {
        return;
    }

    // Load permission cache — hard fail if the cache cannot be loaded.
    // An empty cache would deny all non-super_admin users permanently.
    let cache = get_permission_cache();
    cache.load(&pool).await.expect(
        "FATAL: Failed to load permission cache — refusing to start with empty permissions. \
         Check database connectivity and role_permissions table.",
    );

    let ctx = AppContext { pool };
    CONTEXT.set(ctx).ok();
}

// ============================================================================
// 服务工厂函数
// ============================================================================

/// 获取 BOM 服务
pub fn get_bom_service(ctx: &AppContext) -> impl crate::service::BomService {
    crate::implt::BomServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取 BOM 分类服务
pub fn get_bom_category_service(ctx: &AppContext) -> impl crate::service::BomCategoryService {
    crate::implt::BomCategoryServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取产品服务
pub fn get_product_service(ctx: &AppContext) -> impl crate::service::ProductService {
    crate::implt::ProductServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取分类服务
pub fn get_term_service(ctx: &AppContext) -> impl crate::service::TermService {
    crate::implt::TermServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取仓库服务
pub fn get_warehouse_service(ctx: &AppContext) -> impl crate::service::WarehouseService {
    crate::implt::WarehouseServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取库位服务
pub fn get_location_service(ctx: &AppContext) -> impl crate::service::LocationService {
    crate::implt::LocationServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取库存服务
pub fn get_inventory_service(ctx: &AppContext) -> impl crate::service::InventoryService {
    crate::implt::InventoryServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取产品价格服务
pub fn get_product_price_service(ctx: &AppContext) -> impl crate::service::ProductPriceService {
    crate::implt::ProductPriceServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取人工工序服务
pub fn get_labor_process_service(ctx: &AppContext) -> impl crate::service::LaborProcessService {
    crate::implt::LaborProcessServiceImpl::new(ctx.pool().clone())
}

/// 获取工序字典服务
pub fn get_labor_process_dict_service(ctx: &AppContext) -> impl crate::service::LaborProcessDictService {
    crate::implt::LaborProcessDictServiceImpl::new(ctx.pool().clone())
}

/// 获取工艺路线服务
pub fn get_routing_service(ctx: &AppContext) -> impl crate::service::RoutingService {
    crate::implt::RoutingServiceImpl::new(ctx.pool().clone())
}

/// 获取用户服务
pub fn get_user_service(ctx: &AppContext) -> impl crate::service::UserService {
    crate::implt::UserServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取角色服务
pub fn get_role_service(ctx: &AppContext) -> impl crate::service::RoleService {
    crate::implt::RoleServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取权限服务
pub fn get_permission_service(ctx: &AppContext) -> impl crate::service::PermissionService {
    crate::implt::PermissionServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取部门服务
pub fn get_department_service(ctx: &AppContext) -> impl crate::service::DepartmentService {
    crate::implt::DepartmentServiceImpl::new(Arc::new(ctx.pool().clone()))
}

/// 获取认证服务
pub fn get_auth_service(
    pool: sqlx::PgPool,
    jwt_secret: String,
    jwt_expiration_hours: u64,
    resource_actions: Vec<crate::models::ResourceActionDef>,
) -> impl crate::service::AuthService {
    crate::implt::AuthServiceImpl::new(
        Arc::new(pool),
        jwt_secret,
        jwt_expiration_hours,
        resource_actions,
    )
}

