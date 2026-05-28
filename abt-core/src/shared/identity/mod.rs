pub mod auth_service;
pub mod department_service;
pub mod implt;
pub mod model;
pub mod permission_cache;
pub mod permission_service;
pub mod repo;
pub mod role_service;
pub mod user_service;

// Re-export main types
pub use auth_service::AuthService;
pub use department_service::DepartmentService;
pub use model::{AuthContext, Claims, Department, ResourceActionDef, Role, RoleInfo, RoleWithPermissions, User, UserWithRoles, RESOURCE_ACTION_DEFS};
pub use permission_cache::RolePermissionCache;
pub use permission_service::PermissionService;
pub use role_service::RoleService;
pub use user_service::UserService;

// Re-export implementations
pub use implt::{
    AuthServiceImpl, DepartmentServiceImpl, PermissionServiceImpl, RoleServiceImpl,
    UserServiceImpl,
};

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_user_service(pool: PgPool) -> impl UserService {
    use implt::UserServiceImpl;
    use crate::shared::audit_log::implt::AuditLogServiceImpl;

    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));

    UserServiceImpl::new(pool, audit)
}
