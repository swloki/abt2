use crate::models::ResourceActionDef;

/// 获取所有资源的权限定义
pub fn collect_all_resources() -> Vec<ResourceActionDef> {
    RESOURCES.to_vec()
}

static RESOURCES: &[ResourceActionDef] = &[
    // 产品
    ResourceActionDef { resource_code: "product", resource_name: "产品管理", description: "产品基础信息", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "product", resource_name: "产品管理", description: "产品基础信息", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "product", resource_name: "产品管理", description: "产品基础信息", action: "delete", action_name: "删除" },
    // 分类
    ResourceActionDef { resource_code: "term", resource_name: "分类管理", description: "产品分类", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "term", resource_name: "分类管理", description: "产品分类", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "term", resource_name: "分类管理", description: "产品分类", action: "delete", action_name: "删除" },
    // BOM
    ResourceActionDef { resource_code: "bom", resource_name: "BOM管理", description: "BOM清单", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "bom", resource_name: "BOM管理", description: "BOM清单", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "bom", resource_name: "BOM管理", description: "BOM清单", action: "delete", action_name: "删除" },
    // 仓库
    ResourceActionDef { resource_code: "warehouse", resource_name: "仓库管理", description: "仓库信息", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "warehouse", resource_name: "仓库管理", description: "仓库信息", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "warehouse", resource_name: "仓库管理", description: "仓库信息", action: "delete", action_name: "删除" },
    // 库位
    ResourceActionDef { resource_code: "location", resource_name: "库位管理", description: "仓库库位", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "location", resource_name: "库位管理", description: "仓库库位", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "location", resource_name: "库位管理", description: "仓库库位", action: "delete", action_name: "删除" },
    // 库存
    ResourceActionDef { resource_code: "inventory", resource_name: "库存管理", description: "库存操作", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "inventory", resource_name: "库存管理", description: "库存操作", action: "write", action_name: "编辑" },
    // 价格
    ResourceActionDef { resource_code: "price", resource_name: "价格管理", description: "产品价格", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "price", resource_name: "价格管理", description: "产品价格", action: "write", action_name: "编辑" },
    // 人工工序
    ResourceActionDef { resource_code: "labor_process", resource_name: "工序管理", description: "人工工序", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "labor_process", resource_name: "工序管理", description: "人工工序", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "labor_process", resource_name: "工序管理", description: "人工工序", action: "delete", action_name: "删除" },
    // Excel 导入导出
    ResourceActionDef { resource_code: "excel", resource_name: "Excel导入导出", description: "Excel数据导入导出", action: "read", action_name: "导出" },
    ResourceActionDef { resource_code: "excel", resource_name: "Excel导入导出", description: "Excel数据导入导出", action: "write", action_name: "导入" },
    // 工序字典
    ResourceActionDef { resource_code: "labor_process_dict", resource_name: "工序字典", description: "工序字典管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "labor_process_dict", resource_name: "工序字典", description: "工序字典管理", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "labor_process_dict", resource_name: "工序字典", description: "工序字典管理", action: "delete", action_name: "删除" },
    // 工艺路线
    ResourceActionDef { resource_code: "routing", resource_name: "工艺路线", description: "工艺路线管理", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "routing", resource_name: "工艺路线", description: "工艺路线管理", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "routing", resource_name: "工艺路线", description: "工艺路线管理", action: "delete", action_name: "删除" },
    // 用户管理
    ResourceActionDef { resource_code: "user", resource_name: "用户管理", description: "系统用户", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "user", resource_name: "用户管理", description: "系统用户", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "user", resource_name: "用户管理", description: "系统用户", action: "delete", action_name: "删除" },
    // 角色管理
    ResourceActionDef { resource_code: "role", resource_name: "角色管理", description: "系统角色", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "role", resource_name: "角色管理", description: "系统角色", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "role", resource_name: "角色管理", description: "系统角色", action: "delete", action_name: "删除" },
    // 权限管理
    ResourceActionDef { resource_code: "permission", resource_name: "权限管理", description: "角色权限分配", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "permission", resource_name: "权限管理", description: "角色权限分配", action: "write", action_name: "编辑" },
    // 部门管理
    ResourceActionDef { resource_code: "department", resource_name: "部门管理", description: "组织架构", action: "read", action_name: "查看" },
    ResourceActionDef { resource_code: "department", resource_name: "部门管理", description: "组织架构", action: "write", action_name: "编辑" },
    ResourceActionDef { resource_code: "department", resource_name: "部门管理", description: "组织架构", action: "delete", action_name: "删除" },
];
