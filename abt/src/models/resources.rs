use crate::models::ResourceActionDef;

/// 获取所有资源的权限定义
pub fn collect_all_resources() -> Vec<ResourceActionDef> {
    RESOURCES.to_vec()
}

static RESOURCES: &[ResourceActionDef] = &[
    // 产品
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品基础信息", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品基础信息", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "PRODUCT", resource_name: "产品管理", description: "产品基础信息", action: "DELETE", action_name: "删除" },
    // 分类
    ResourceActionDef { resource_code: "TERM", resource_name: "分类管理", description: "产品分类", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "TERM", resource_name: "分类管理", description: "产品分类", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "TERM", resource_name: "分类管理", description: "产品分类", action: "DELETE", action_name: "删除" },
    // BOM
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM清单", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM清单", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "BOM", resource_name: "BOM管理", description: "BOM清单", action: "DELETE", action_name: "删除" },
    // 仓库
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库信息", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库信息", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "WAREHOUSE", resource_name: "仓库管理", description: "仓库信息", action: "DELETE", action_name: "删除" },
    // 库位
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "仓库库位", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "仓库库位", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "LOCATION", resource_name: "库位管理", description: "仓库库位", action: "DELETE", action_name: "删除" },
    // 库存
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存操作", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "INVENTORY", resource_name: "库存管理", description: "库存操作", action: "WRITE", action_name: "编辑" },
    // 价格
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "产品价格", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "PRICE", resource_name: "价格管理", description: "产品价格", action: "WRITE", action_name: "编辑" },
    // 人工工序
    ResourceActionDef { resource_code: "LABOR_PROCESS", resource_name: "工序管理", description: "人工工序", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "LABOR_PROCESS", resource_name: "工序管理", description: "人工工序", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "LABOR_PROCESS", resource_name: "工序管理", description: "人工工序", action: "DELETE", action_name: "删除" },
    // Excel 导入导出
    ResourceActionDef { resource_code: "EXCEL", resource_name: "Excel导入导出", description: "Excel数据导入导出", action: "READ", action_name: "导出" },
    ResourceActionDef { resource_code: "EXCEL", resource_name: "Excel导入导出", description: "Excel数据导入导出", action: "WRITE", action_name: "导入" },
    // 工序字典
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "LABOR_PROCESS_DICT", resource_name: "工序字典", description: "工序字典管理", action: "DELETE", action_name: "删除" },
    // 工艺路线
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "ROUTING", resource_name: "工艺路线", description: "工艺路线管理", action: "DELETE", action_name: "删除" },
    // 用户管理
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "系统用户", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "系统用户", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "USER", resource_name: "用户管理", description: "系统用户", action: "DELETE", action_name: "删除" },
    // 角色管理
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "系统角色", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "系统角色", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "ROLE", resource_name: "角色管理", description: "系统角色", action: "DELETE", action_name: "删除" },
    // 权限管理
    ResourceActionDef { resource_code: "PERMISSION", resource_name: "权限管理", description: "角色权限分配", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "PERMISSION", resource_name: "权限管理", description: "角色权限分配", action: "WRITE", action_name: "编辑" },
    // 部门管理
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "组织架构", action: "READ", action_name: "查看" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "组织架构", action: "WRITE", action_name: "编辑" },
    ResourceActionDef { resource_code: "DEPARTMENT", resource_name: "部门管理", description: "组织架构", action: "DELETE", action_name: "删除" },
    // BOM 成本
    ResourceActionDef { resource_code: "BOM_COST", resource_name: "BOM成本", description: "BOM成本查看", action: "READ", action_name: "查看" },
    // BOM 人工成本（单独查看）
    ResourceActionDef { resource_code: "BOM_LABOR_COST", resource_name: "BOM人工成本", description: "BOM人工成本单独查看", action: "READ", action_name: "查看" },
];
