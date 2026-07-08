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

/// 30 resources x 4 actions = 120 permission entries
pub static RESOURCE_ACTION_DEFS: &[ResourceActionDef] = &[
    // ── 基础数据 ──
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "客户管理", description: "客户管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "客户管理", description: "客户管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "客户管理", description: "客户管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "CUSTOMER", resource_name: "客户管理", description: "客户管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "SUPPLIER", resource_name: "供应商管理", description: "供应商管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "SUPPLIER", resource_name: "供应商管理", description: "供应商管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "SUPPLIER", resource_name: "供应商管理", description: "供应商管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "SUPPLIER", resource_name: "供应商管理", description: "供应商管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "产品分类", description: "产品分类管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "产品分类", description: "产品分类管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "产品分类", description: "产品分类管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "CATEGORY", resource_name: "产品分类", description: "产品分类管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM分类", description: "BOM分类管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM分类", description: "BOM分类管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM分类", description: "BOM分类管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "BOM_CATEGORY", resource_name: "BOM分类", description: "BOM分类管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "delete", action_name: "删除" },
    // ── 计件单价（R-13：定价影响全员工资，独立闸门，非 WORK_ORDER update）──
    ResourceActionDef { resource_code: "BOM_STEP_PRICE", resource_name: "计件单价", description: "BOM 工序计件单价管理（影响全员工资）", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "BOM_STEP_PRICE", resource_name: "计件单价", description: "BOM 工序计件单价管理（影响全员工资）", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "BOM_STEP_PRICE", resource_name: "计件单价", description: "BOM 工序计件单价管理（影响全员工资）", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "BOM_STEP_PRICE", resource_name: "计件单价", description: "BOM 工序计件单价管理（影响全员工资）", action: "delete", action_name: "删除" },
    // ── 仓储 ──
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "库位管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "库位管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "库位管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "库位管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "价格管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "价格管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "价格管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "价格管理", action: "delete", action_name: "删除" },
    // ── 销售 ──
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "销售订单", description: "销售订单管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "销售订单", description: "销售订单管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "销售订单", description: "销售订单管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "SALES_ORDER", resource_name: "销售订单", description: "销售订单管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "发货管理", description: "发货管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "发货管理", description: "发货管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "发货管理", description: "发货管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "SHIPPING", resource_name: "发货管理", description: "发货管理", action: "delete", action_name: "删除" },
    // ── 采购 ──
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "采购订单", description: "采购订单管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "采购订单", description: "采购订单管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "采购订单", description: "采购订单管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PURCHASE_ORDER", resource_name: "采购订单", description: "采购订单管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PURCHASE_QUOTATION", resource_name: "采购报价", description: "采购报价管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PURCHASE_QUOTATION", resource_name: "采购报价", description: "采购报价管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PURCHASE_QUOTATION", resource_name: "采购报价", description: "采购报价管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PURCHASE_QUOTATION", resource_name: "采购报价", description: "采购报价管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PURCHASE_RETURN", resource_name: "采购退货", description: "采购退货管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PURCHASE_RETURN", resource_name: "采购退货", description: "采购退货管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PURCHASE_RETURN", resource_name: "采购退货", description: "采购退货管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PURCHASE_RETURN", resource_name: "采购退货", description: "采购退货管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PURCHASE_RECON", resource_name: "采购对账", description: "采购对账管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PURCHASE_RECON", resource_name: "采购对账", description: "采购对账管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PURCHASE_RECON", resource_name: "采购对账", description: "采购对账管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PURCHASE_RECON", resource_name: "采购对账", description: "采购对账管理", action: "delete", action_name: "删除" },
    // ── 生产制造 ──
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "生产工单", description: "生产工单管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "生产工单", description: "生产工单管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "生产工单", description: "生产工单管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "WORK_ORDER", resource_name: "生产工单", description: "生产工单管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "生产检验", description: "生产检验管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "生产检验", description: "生产检验管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "生产检验", description: "生产检验管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "INSPECTION", resource_name: "生产检验", description: "生产检验管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "COST", resource_name: "成本管理", description: "成本管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "COST", resource_name: "成本管理", description: "成本管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "COST", resource_name: "成本管理", description: "成本管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "人工成本", description: "人工成本管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "人工成本", description: "人工成本管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "人工成本", description: "人工成本管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "LABOR_COST", resource_name: "人工成本", description: "人工成本管理", action: "delete", action_name: "删除" },
    // ── 委外管理 ──
    ResourceActionDef { resource_code: "OM", resource_name: "委外管理", description: "委外管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "OM", resource_name: "委外管理", description: "委外管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "OM", resource_name: "委外管理", description: "委外管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "OM", resource_name: "委外管理", description: "委外管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "OUTSOURCING", resource_name: "委外单", description: "委外单管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "OUTSOURCING", resource_name: "委外单", description: "委外单管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "OUTSOURCING", resource_name: "委外单", description: "委外单管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "OUTSOURCING", resource_name: "委外单", description: "委外单管理", action: "delete", action_name: "删除" },
    // ── 质量管理 ──
    ResourceActionDef { resource_code: "QMS", resource_name: "质量管理", description: "质量管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "QMS", resource_name: "质量管理", description: "质量管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "QMS", resource_name: "质量管理", description: "质量管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "QMS", resource_name: "质量管理", description: "质量管理", action: "delete", action_name: "删除" },
    // ── 财务管理 ──
    ResourceActionDef { resource_code: "FMS", resource_name: "财务管理", description: "财务管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "FMS", resource_name: "财务管理", description: "财务管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "FMS", resource_name: "财务管理", description: "财务管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "FMS", resource_name: "财务管理", description: "财务管理", action: "delete", action_name: "删除" },
    // ── 总账（GL）──
    ResourceActionDef { resource_code: "GL", resource_name: "总账管理", description: "总账管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "GL", resource_name: "总账管理", description: "总账管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "GL", resource_name: "总账管理", description: "总账管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "GL", resource_name: "总账管理", description: "总账管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "MISC_REQUEST", resource_name: "杂项申请", description: "杂项申请管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "MISC_REQUEST", resource_name: "杂项申请", description: "杂项申请管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "MISC_REQUEST", resource_name: "杂项申请", description: "杂项申请管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "MISC_REQUEST", resource_name: "杂项申请", description: "杂项申请管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "PAYMENT_REQUEST", resource_name: "付款申请", description: "付款申请管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "PAYMENT_REQUEST", resource_name: "付款申请", description: "付款申请管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "PAYMENT_REQUEST", resource_name: "付款申请", description: "付款申请管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "PAYMENT_REQUEST", resource_name: "付款申请", description: "付款申请管理", action: "delete", action_name: "删除" },
    // ── 系统管理 ──
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "用户管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "用户管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "用户管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "用户管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "角色管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "角色管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "角色管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "角色管理", action: "delete", action_name: "删除" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "部门管理", action: "create", action_name: "创建" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "部门管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "部门管理", action: "update", action_name: "编辑" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "部门管理", action: "delete", action_name: "删除" },
 ];
