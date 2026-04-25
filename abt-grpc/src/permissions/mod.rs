use crate::generated::abt::v1::{Action, Resource};

/// Trait for converting proto-generated permission enums to runtime strings.
///
/// Proto enums use SCREAMING_SNAKE_CASE, and `as_str_name()` returns them as-is
/// (e.g., `"WAREHOUSE"`, `"READ"`), matching the database and cache format.
pub trait PermissionCode {
    fn code(&self) -> String;
}

impl PermissionCode for Resource {
    fn code(&self) -> String {
        self.as_str_name().to_string()
    }
}

impl PermissionCode for Action {
    fn code(&self) -> String {
        self.as_str_name().to_string()
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
