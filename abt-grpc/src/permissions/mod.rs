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
// Scoped Permission Check Functions
// ============================================================================

/// Main permission check entry point, called by the `require_permission` macro.
///
/// Dispatches to system or business resource check based on resource type.
pub fn check_permission_for_resource(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
    department_id: Option<i64>,
) -> Result<(), String> {
    if abt::is_system_resource(resource_code) {
        check_system_permission(auth, resource_code, action_code)
    } else {
        check_business_permission(auth, resource_code, action_code, department_id)
    }
}

/// Check permission for system resources (user, role, permission, department, excel).
///
/// System resources are governed by system_role:
/// - super_admin: full access to all system resources
/// - user: read-only access to user, department, permission
fn check_system_permission(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
) -> Result<(), String> {
    if auth.is_super_admin() {
        return Ok(());
    }

    // Non-admin users get read-only access to select system resources
    let user_permissions = ["user:read", "department:read", "permission:read", "role:read"];
    let required = format!("{}:{}", resource_code, action_code);
    if user_permissions.contains(&required.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "No system permission for {}:{}",
            resource_code, action_code
        ))
    }
}

/// Check permission for business resources (product, term, bom, warehouse, etc.).
///
/// Flow:
/// 1. Super admin with no dept_roles -> full access (system-level admin)
/// 2. Extract department_id from gRPC metadata (passed as argument)
/// 3. Check user belongs to this department
/// 4. Super admin who belongs to the department -> allow
/// 5. Check department has access to this resource type
/// 6. Check user's roles in this department have the required permission via cache
fn check_business_permission(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
    department_id: Option<i64>,
) -> Result<(), String> {
    // Super admin with no dept assignments = full access
    if auth.is_super_admin() {
        if auth.dept_roles.is_empty() {
            return Ok(());
        }
        // Super admin with dept assignments still needs dept context
        // but skips role/permission checks after membership verification
    }

    // Must have a department context
    let dept_id = department_id.ok_or_else(|| {
        "department_id is required for business resource operations".to_string()
    })?;

    // Step 1: Department membership check
    if !auth.belongs_to_department(dept_id) {
        return Err("User does not belong to the specified department".to_string());
    }

    // Super admin who belongs to the department -> allow
    if auth.is_super_admin() {
        return Ok(());
    }

    // Step 2: Department resource visibility check
    let has_resource = {
        let cache = abt::get_dept_resource_access_cache();
        cache.has_resource(dept_id, resource_code)
    };
    if !has_resource {
        return Err("Department does not have access to the specified resource".to_string());
    }

    // Step 3: Check role permissions via cache
    let role_ids = auth.get_dept_role_ids(dept_id);
    if role_ids.is_empty() {
        return Err("User has no roles in the specified department".to_string());
    }

    let cache = abt::get_permission_cache();
    let has_perm = cache.has_permission(&role_ids, resource_code, action_code);

    if has_perm {
        Ok(())
    } else {
        Err(format!(
            "No permission for {}:{} in current department",
            resource_code, action_code
        ))
    }
}

#[cfg(test)]
mod tests;
