use crate::generated::abt::v1::{Action, Resource};

/// Trait for converting proto-generated permission enums to lowercase runtime strings.
///
/// The proto enums use SCREAMING_SNAKE_CASE (e.g., `WAREHOUSE`, `READ`), but the
/// runtime permission system uses lowercase strings (e.g., `"warehouse"`, `"read"`)
/// for JWT claims and `check_permission` matching. This trait bridges the two.
pub trait PermissionCode {
    fn code(&self) -> &'static str;
}

impl PermissionCode for Resource {
    fn code(&self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Term => "term",
            Self::Bom => "bom",
            Self::Warehouse => "warehouse",
            Self::Location => "location",
            Self::Inventory => "inventory",
            Self::Price => "price",
            Self::LaborProcess => "labor_process",
            Self::User => "user",
            Self::Role => "role",
            Self::Permission => "permission",
            Self::Department => "department",
            Self::Excel => "excel",
        }
    }
}

impl PermissionCode for Action {
    fn code(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
        }
    }
}

// ============================================================================
// Permission Check Functions
// ============================================================================

/// Main permission check entry point, called by the `require_permission` macro.
///
/// Flow:
/// 1. Super admin → full access
/// 2. Check user's roles have the required permission via RolePermissionCache
pub fn check_permission_for_resource(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
) -> Result<(), String> {
    if auth.is_super_admin() {
        return Ok(());
    }

    let role_ids = &auth.role_ids;
    if role_ids.is_empty() {
        return Err(format!("No permission for {}:{}", resource_code, action_code));
    }

    let cache = abt::get_permission_cache();
    let has_perm = cache.has_permission(role_ids, resource_code, action_code);

    if has_perm {
        Ok(())
    } else {
        Err(format!("No permission for {}:{}", resource_code, action_code))
    }
}

#[cfg(test)]
mod tests;
