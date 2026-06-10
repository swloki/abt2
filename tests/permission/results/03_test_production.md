# test_production 权限测试结果

**测试用户**: test_production  
**测试日期**: 2026-06-10  
**测试角色**: 生产主管 (production_supervisor)

## 用户权限配置

| 资源编码 | 权限 |
|---------|------|
| WORK_ORDER | create, read, update (无 delete) |
| INSPECTION | create, read, update (无 delete) |
| LABOR_COST | read, update (无 create/delete) |
| COST | read |
| PRODUCT | read |
| BOM | read |

## 关键发现: 资源编码不匹配

**严重问题**: 种子脚本分配的权限资源编码与服务器实际检查的资源编码不匹配。

| 种子脚本分配的资源编码 | 服务器实际检查的资源编码 | 影响 |
|----------------------|----------------------|------|
| WORK_ORDER | **MES** | MES 所有页面因缺少 MES:read 被 403 |
| INSPECTION | **MES** | 生产报检页面因缺少 MES:read 被 403 |
| LABOR_COST | **MES** (推测) | 计件工资页面因缺少 MES:read 被 403 |
| COST | 未知 | 无独立成本页面可测试 |
| PRODUCT | PRODUCT | 匹配 - 页面正常加载 |
| BOM | BOM | 匹配 - 页面正常加载 |

**结论**: MES 模块所有页面统一使用 "MES" 作为资源编码，而非按功能拆分为 "WORK_ORDER"、"INSPECTION" 等。种子脚本需更新为使用 "MES" 资源编码。

## 1. MES 生产管理页面

| 测试ID | 页面 | URL | 状态码 | 服务器检查的资源 | 结果 |
|--------|------|-----|--------|----------------|------|
| TP-PROD-MES-01 | 生产总览 | /admin/mes | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-02 | 生产计划 | /admin/mes/plans | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-03 | 工单管理 | /admin/mes/orders | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-04 | 生产批次 | /admin/mes/batches | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-05 | 报工记录 | /admin/mes/reports | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-06 | 计件工资 | /admin/mes/wages | 403 | MES:read | BLOCKED (预期: 应可访问) |
| TP-PROD-MES-07 | 生产报检 | /admin/mes/inspections | 403 | MES:read | BLOCKED (预期: 应可访问) |

**结果: 7/7 全部被拦截** — 因为权限编码不匹配。所有 MES 页面检查 `MES:read`，但用户被分配了 `WORK_ORDER:read` 和 `INSPECTION:read`。

## 2. 主数据页面 (有权限)

| 测试ID | 页面 | URL | 状态码 | 服务器检查的资源 | 结果 |
|--------|------|-----|--------|----------------|------|
| TP-PROD-MD-01 | BOM 管理 | /admin/md/boms | 200 | BOM:read | PASS - 页面正常加载 |
| TP-PROD-MD-02 | 产品管理 | /admin/md/products | 200 | PRODUCT:read | PASS - 页面正常加载 |
| TP-PROD-MD-03 | 主数据总览 | /admin/md | 200 | (主数据首页) | PASS - 页面正常加载 |

## 3. 主数据页面 (无权限)

| 测试ID | 页面 | URL | 状态码 | 服务器检查的资源 | 结果 |
|--------|------|-----|--------|----------------|------|
| TP-PROD-MD-04 | 产品分类 | /admin/md/categories | 403 | CATEGORY:read | PASS - 正确拦截 |
| TP-PROD-MD-05 | 供应商管理 | /admin/md/suppliers | 403 | SUPPLIER:read | PASS - 正确拦截 |

## 4. 未授权页面访问测试

| 测试ID | 页面 | URL | 状态码 | 服务器检查的资源 | 结果 |
|--------|------|-----|--------|----------------|------|
| TP-PROD-SEC-01 | 库存总览 | /admin/wms | 403 | WMS:read | PASS - 正确拦截 |
| TP-PROD-SEC-02 | 仓库管理 | /admin/wms/warehouses | 403 | WAREHOUSE:read | PASS - 正确拦截 |
| TP-PROD-SEC-03 | 销售订单 | /admin/orders | 403 | SALES_ORDER:read | PASS - 正确拦截 |
| TP-PROD-SEC-04 | 用户管理 | /admin/system/users | 403 | USER:read | PASS - 正确拦截 |
| TP-PROD-SEC-05 | 客户管理 | /admin/customers | 403 | CUSTOMER:read | PASS - 正确拦截 |
| TP-PROD-SEC-06 | 采购管理 | /admin/purchase | 403 | PURCHASE_ORDER:read | PASS - 正确拦截 |
| TP-PROD-SEC-07 | 角色管理 | /admin/system/roles | 403 | ROLE:read | PASS - 正确拦截 |
| TP-PROD-SEC-08 | 权限配置 | /admin/system/permissions | 403 | ROLE:read | PASS - 正确拦截 |
| TP-PROD-SEC-09 | QMS 报检 | /admin/qms/inspections | 404 | (页面不存在) | N/A |
| TP-PROD-SEC-10 | 销售总览(首页) | /admin | 200 | (默认首页) | PASS - 允许访问 |

## 5. 按钮级权限测试

### 5.1 BOM 管理页面 (BOM:read - 只读)

| 测试ID | 按钮 | 预期 | 实际 | 结果 |
|--------|------|------|------|------|
| TP-PROD-BTN-01 | 新建 BOM | 隐藏 | 未出现 (页面无新建按钮) | PASS |
| TP-PROD-BTN-02 | 查看 | 显示 | 显示 | PASS |
| TP-PROD-BTN-03 | 查看成本 | 显示 | 显示 | PASS |
| TP-PROD-BTN-04 | 编辑 | 隐藏 | 未出现 (仅查看和成本) | PASS |

**注**: BOM 列表页面操作列只显示"查看"和"查看成本"按钮，无编辑/删除按钮。符合只读权限预期。

### 5.2 产品管理页面 (PRODUCT:read - 只读)

| 测试ID | 按钮 | 预期 | 实际 | 结果 |
|--------|------|------|------|------|
| TP-PROD-BTN-05 | 新建产品 | 隐藏 | **显示** | FAIL - 按钮过滤失效 |
| TP-PROD-BTN-06 | 编辑 | 隐藏 | **显示** | FAIL - 按钮过滤失效 |
| TP-PROD-BTN-07 | 复制 | 隐藏 | **显示** | FAIL - 按钮过滤失效 |
| TP-PROD-BTN-08 | 删除 | 隐藏 | **显示** | FAIL - 按钮过滤失效 |
| TP-PROD-BTN-09 | 设置价格 | 隐藏 | **显示** | FAIL - 按钮过滤失效 |

**产品页面按钮权限控制完全失效**: 用户仅有 PRODUCT:read 权限，但"新建产品"、"编辑"、"复制"、"删除"、"设置价格"等按钮全部可见。

## 6. 侧边栏 NavFilter 测试

| 测试ID | 模块 | 预期 | 实际 | 结果 |
|--------|------|------|------|------|
| TP-PROD-NAV-01 | 销售 | 隐藏 | **显示** | FAIL |
| TP-PROD-NAV-02 | 采购 | 隐藏 | **显示** | FAIL |
| TP-PROD-NAV-03 | 库存 | 隐藏 | **显示** | FAIL |
| TP-PROD-NAV-04 | 生产 | 显示 | 显示 | PASS |
| TP-PROD-NAV-05 | 主数据 | 显示 | 显示 | PASS |
| TP-PROD-NAV-06 | 系统 | 隐藏 | **显示** | FAIL |

**侧边栏 NavFilter 完全失效**: 所有 6 个模块全部显示，不论用户是否有权限。

## 7. 已知问题汇总

### 关键 BUG

| 编号 | 问题 | 严重程度 | 影响范围 |
|------|------|---------|---------|
| BUG-NAV-01 | NavFilter 侧边栏不过滤模块 | 高 | 所有非管理员用户 |
| BUG-BTN-01 | 产品页面按钮权限过滤失效 | 高 | 产品管理页面 |
| BUG-RES-01 | MES 资源编码不匹配 | 高 | 所有 MES 页面 |

### 资源编码映射表 (服务器实际值)

| URL 路径前缀 | 服务器检查的资源编码 |
|-------------|-------------------|
| /admin/mes/* | MES |
| /admin/md/boms | BOM |
| /admin/md/products | PRODUCT |
| /admin/md/categories | CATEGORY |
| /admin/md/suppliers | SUPPLIER |
| /admin/wms | WMS |
| /admin/wms/warehouses | WAREHOUSE |
| /admin/orders | SALES_ORDER |
| /admin/customers | CUSTOMER |
| /admin/system/users | USER |
| /admin/system/roles | ROLE |
| /admin/purchase/* | PURCHASE_ORDER |

## 8. 总结

| 类别 | 通过 | 失败 | 总数 |
|------|------|------|------|
| MES 页面访问 | 0 | 7 | 7 |
| 主数据有权限页面 | 3 | 0 | 3 |
| 主数据无权限页面 | 2 | 0 | 2 |
| 未授权页面拦截 | 8 | 0 | 8 |
| 按钮权限 | 4 | 5 | 9 |
| NavFilter | 2 | 4 | 6 |
| **合计** | **19** | **16** | **35** |

**核心问题**: 
1. MES 模块统一使用 "MES" 资源编码，种子脚本分配的 "WORK_ORDER"、"INSPECTION"、"LABOR_COST" 编码无法匹配，导致所有 MES 页面被拦截
2. 服务器端 403 拦截机制工作正常
3. 前端 NavFilter 和按钮权限过滤仍存在已知问题
