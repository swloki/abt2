use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct User {
    pub user_id: i64,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub is_active: bool,
    pub is_super_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("password_hash", &"***")
            .field("display_name", &self.display_name)
            .field("is_active", &self.is_active)
            .field("is_super_admin", &self.is_super_admin)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Role {
    pub role_id: i64,
    pub role_name: String,
    pub role_code: String,
    pub is_system_role: bool,
    pub parent_role_id: Option<i64>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Department
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Department {
    pub department_id: i64,
    pub department_name: String,
    pub department_code: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// JWT Claims
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// user_id
    pub sub: i64,
    pub username: String,
    pub display_name: String,
    /// "super_admin" | "user"
    pub system_role: String,
    pub role_ids: Vec<i64>,
    pub role_codes: Vec<String>,
    pub department_ids: Vec<i64>,
    pub iss: String,
    pub exp: u64,
    pub iat: u64,
}

impl Claims {
    pub fn is_super_admin(&self) -> bool {
        self.system_role == "super_admin"
            || self.role_codes.iter().any(|c| c == "super_admin")
    }
}
// ---------------------------------------------------------------------------
// AuthContext — gRPC request-level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub system_role: String,
    pub role_ids: Vec<i64>,
    pub role_codes: Vec<String>,
    pub department_ids: Vec<i64>,
}

impl AuthContext {
    pub fn is_super_admin(&self) -> bool {
        self.system_role == "super_admin"
            || self.role_codes.iter().any(|c| c == "super_admin")
    }

    pub fn has_role(&self, role_id: i64) -> bool {
        self.role_ids.contains(&role_id)
    }
}

// ---------------------------------------------------------------------------
// UserWithRoles — composite for API responses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RoleInfo {
    pub role_id: i64,
    pub role_name: String,
    pub role_code: String,
}

#[derive(Debug, Clone)]
pub struct UserWithRoles {
    pub user: User,
    pub roles: Vec<RoleInfo>,
}

// ---------------------------------------------------------------------------
// RoleWithPermissions — composite for API responses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RoleWithPermissions {
    pub role: Role,
    pub permissions: Vec<String>,
    pub inherited_permissions: Vec<String>,
}

// ---------------------------------------------------------------------------
// ResourceActionDef — permission resource/action definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResourceActionDef {
    pub resource_code: &'static str,
    pub resource_name: &'static str,
    pub description: &'static str,
    pub action: &'static str,
    pub action_name: &'static str,
}

/// 19 resources x 4 actions = 76 permission entries
pub static RESOURCE_ACTION_DEFS: &[ResourceActionDef] = &[
    // CUSTOMER
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "Customer", description: "Customer management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "Customer", description: "Customer management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "Customer", description: "Customer management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "Customer", description: "Customer management", action: "delete", action_name: "Delete" },
    // PRODUCT
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "Product", description: "Product management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "Product", description: "Product management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "Product", description: "Product management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "Product", description: "Product management", action: "delete", action_name: "Delete" },
    // CATEGORY
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "Category", description: "Category management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "Category", description: "Category management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "Category", description: "Category management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "Category", description: "Category management", action: "delete", action_name: "Delete" },
    // BOM
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM", description: "BOM management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM", description: "BOM management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM", description: "BOM management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM", description: "BOM management", action: "delete", action_name: "Delete" },
    // BOM_CATEGORY
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM Category", description: "BOM category management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM Category", description: "BOM category management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM Category", description: "BOM category management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM Category", description: "BOM category management", action: "delete", action_name: "Delete" },
    // WAREHOUSE
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "Warehouse", description: "Warehouse management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "Warehouse", description: "Warehouse management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "Warehouse", description: "Warehouse management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "Warehouse", description: "Warehouse management", action: "delete", action_name: "Delete" },
    // LOCATION
    ResourceActionDef { resource_code: "LOCATION", resource_name: "Location", description: "Storage location management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "Location", description: "Storage location management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "Location", description: "Storage location management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "Location", description: "Storage location management", action: "delete", action_name: "Delete" },
    // INVENTORY
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "Inventory", description: "Inventory management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "Inventory", description: "Inventory management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "Inventory", description: "Inventory management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "Inventory", description: "Inventory management", action: "delete", action_name: "Delete" },
    // PRICE
    ResourceActionDef { resource_code: "PRICE", resource_name: "Price", description: "Price management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "Price", description: "Price management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "Price", description: "Price management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "Price", description: "Price management", action: "delete", action_name: "Delete" },
    // SALES_ORDER
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "Sales Order", description: "Sales order management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "Sales Order", description: "Sales order management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "Sales Order", description: "Sales order management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "Sales Order", description: "Sales order management", action: "delete", action_name: "Delete" },
    // PURCHASE_ORDER
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "Purchase Order", description: "Purchase order management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "Purchase Order", description: "Purchase order management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "Purchase Order", description: "Purchase order management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "Purchase Order", description: "Purchase order management", action: "delete", action_name: "Delete" },
    // WORK_ORDER
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "Work Order", description: "Work order management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "Work Order", description: "Work order management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "Work Order", description: "Work order management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "Work Order", description: "Work order management", action: "delete", action_name: "Delete" },
    // INSPECTION
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "Inspection", description: "Quality inspection management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "Inspection", description: "Quality inspection management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "Inspection", description: "Quality inspection management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "Inspection", description: "Quality inspection management", action: "delete", action_name: "Delete" },
    // COST
    ResourceActionDef { resource_code: "COST", resource_name: "Cost", description: "Cost management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "COST", resource_name: "Cost", description: "Cost management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "COST", resource_name: "Cost", description: "Cost management", action: "update", action_name: "Update" },
    // LABOR_COST
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "Labor Cost", description: "Labor cost management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "Labor Cost", description: "Labor cost management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "Labor Cost", description: "Labor cost management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "Labor Cost", description: "Labor cost management", action: "delete", action_name: "Delete" },
    // USER
    ResourceActionDef { resource_code: "USER", resource_name: "User", description: "User management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "USER", resource_name: "User", description: "User management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "USER", resource_name: "User", description: "User management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "USER", resource_name: "User", description: "User management", action: "delete", action_name: "Delete" },
    // ROLE
    ResourceActionDef { resource_code: "ROLE", resource_name: "Role", description: "Role management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "Role", description: "Role management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "Role", description: "Role management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "Role", description: "Role management", action: "delete", action_name: "Delete" },
    // DEPARTMENT
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "Department", description: "Department management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "Department", description: "Department management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "Department", description: "Department management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "Department", description: "Department management", action: "delete", action_name: "Delete" },
    // SHIPPING
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "Shipping", description: "Shipping request management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "Shipping", description: "Shipping request management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "Shipping", description: "Shipping request management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "Shipping", description: "Shipping request management", action: "delete", action_name: "Delete" },
    // FMS
    ResourceActionDef { resource_code: "FMS", resource_name: "Financial Management", description: "Financial management", action: "create", action_name: "Create" },
    ResourceActionDef { resource_code: "FMS", resource_name: "Financial Management", description: "Financial management", action: "read", action_name: "Read" },
    ResourceActionDef { resource_code: "FMS", resource_name: "Financial Management", description: "Financial management", action: "update", action_name: "Update" },
    ResourceActionDef { resource_code: "FMS", resource_name: "Financial Management", description: "Financial management", action: "delete", action_name: "Delete" },
 ];
