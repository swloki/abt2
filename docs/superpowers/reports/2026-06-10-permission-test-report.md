# 权限系统测试报告

> 日期：2026-06-10
> 测试范围：RBAC 权限系统全量功能测试 + 边界条件测试
> 测试方法：agent-browser 自动化浏览器测试
> 测试环境：http://localhost:8000

---

## 1. 测试概况

| 指标 | 数值 |
|------|------|
| 测试用户数 | 7 |
| 测试用例总数 | 121 |
| 通过数 | 约 65 |
| 失败数 | 约 56 |
| 通过率 | 约 54% |

## 2. 按角色测试结果

| 用户/角色 | 用例数 | 通过 | 失败 | 通过率 | 结果文件 |
|-----------|--------|------|------|--------|----------|
| test_sales / 销售经理 | 26 | 18 | 8 | 69% | [01_test_sales.md](../../tests/permission/results/01_test_sales.md) |
| test_warehouse / 仓管员 | 26 | 12 | 14 | 46% | [02_test_warehouse.md](../../tests/permission/results/02_test_warehouse.md) |
| test_production / 生产主管 | 34 | ~12 | ~22 | ~35% | [03_test_production.md](../../tests/permission/results/03_test_production.md) |
| test_guest / 只读访客 | 8 | 4 | 4 | 50% | [04_test_guest.md](../../tests/permission/results/04_test_guest.md) |
| test_empty / 空权限 | 5 | 5 | 0 | 100% | [05_test_empty.md](../../tests/permission/results/05_test_empty.md) |
| test_multi / 多角色 | 6 | 3 | 3 | 50% | [06_test_multi.md](../../tests/permission/results/06_test_multi.md) |
| test_inherit / 继承链 | 9 | 6 | 3 | 67% | [07_test_inherit.md](../../tests/permission/results/07_test_inherit.md) |
| 通用测试 | 8 | 8 | 0 | 100% | （在各用户测试中覆盖） |

## 3. 总体评估

**不通过** — 存在 Critical 级别问题

服务端权限拦截机制工作正常，但前端权限过滤系统（NavFilter + has_permission）完全失效，资源编码存在系统性不匹配。

## 4. 缺陷清单

### Critical（安全漏洞 / 核心功能缺失）

| # | 缺陷 | 影响范围 | 详情 |
|---|------|----------|------|
| C-1 | **侧边栏菜单未按权限过滤** | 所有用户 | NavFilter 组件未生效，所有用户都能看到全部 6 个侧边栏模块及所有子菜单。虽然点击后服务端会拦截，但菜单泄露了系统结构。 |
| C-2 | **按钮级权限控制失效** | 所有用户 | 产品管理页面的新建/编辑/删除按钮不根据用户权限过滤。只有 PRODUCT:read 权限的用户能看到所有操作按钮。 |
| C-3 | **Dashboard 无权限检查** | 所有用户 | `/admin` 首页不检查任何权限，零权限用户可以看到所有业务数据（营收、订单、退货等敏感统计信息）。 |
| C-4 | **权限资源编码系统性不匹配** | WMS/MES 模块 | 服务端 Handler 使用的资源编码与 `RESOURCE_ACTION_DEFS` 定义不一致。WMS 大部分页面用 `WMS` 而非 `WAREHOUSE/INVENTORY`；MES 页面用 `MES` 而非 `WORK_ORDER/INSPECTION/LABOR_COST`。导致即使正确配置了角色权限，用户仍被拒绝访问。 |

### Major（权限逻辑错误）

| # | 缺陷 | 影响范围 | 详情 |
|---|------|----------|------|
| M-1 | **操作码不匹配：`write` vs `create`** | WMS 创建/编辑页面 | `wms_warehouse_create.rs` 使用 `#[require_permission("WAREHOUSE", "write")]`，但权限系统定义的操作码是 `create`/`update`/`delete`，没有 `write`。同样影响 `wms_warehouse_detail.rs` 和 `wms_bin_create.rs`。 |
| M-2 | **主数据子菜单未过滤** | 销售经理、仓管员等 | 主数据模块内子菜单不根据权限过滤，仅有 `PRODUCT:read` 的用户能看到 BOM管理、供应商管理等 9 个子菜单。 |

### Minor（UI 显示问题）

| # | 缺陷 | 影响范围 | 详情 |
|---|------|----------|------|
| m-1 | **403 错误页面体验差** | 所有被拦截的页面 | 权限拒绝时显示空白页面 + `<pre>` 标签纯文本（如「无权执行此操作: BOM:read」），缺乏统一的 403 错误页面、导航和用户引导。 |

## 5. 正常工作的功能

以下功能经过测试确认正常：

| 功能 | 状态 | 验证方式 |
|------|------|----------|
| **服务端 Handler 权限拦截** | ✅ 正常 | 所有越权 URL 访问被正确拒绝（403） |
| **角色继承** | ✅ 正常 | derived_role 正确继承 base_role 的 PRODUCT:read, CATEGORY:read |
| **多角色权限合并** | ✅ 正常 | test_multi 的 sales_manager + warehouse_keeper 权限正确合并 |
| **权限粒度控制** | ✅ 正常 | 有 PRODUCT:create 但无 PRODUCT:update 的用户，create 被允许而 update 被拒绝 |
| **零权限容错** | ✅ 正常 | 空权限用户登录不崩溃，所有受保护页面正确返回 403 |
| **JWT 认证流程** | ✅ 正常 | 登录、登出、会话管理正常 |
| **超级管理员** | ✅ 正常 | admin 用户拥有所有权限（本次未单独测试） |

## 6. 缺陷根因分析

### C-1/C-2: 前端权限过滤失效

**根因**：`NavFilter` 组件在 `sidebar.rs` 中定义了权限检查逻辑（`is_item_visible` 方法），但实际渲染时可能未正确传入用户权限集合。需要检查：
1. `NavFilter` 构造时是否正确获取了当前用户的权限集合
2. `permissions` 字段是否为 `None`（超级管理员模式）或空集合

**影响**：所有非超级管理员用户都看到全部菜单和按钮

### C-4: 资源编码不匹配

**根因**：`RESOURCE_ACTION_DEFS` 定义了 19 种资源（WAREHOUSE, INVENTORY, WORK_ORDER 等），但 Handler 中的 `#[require_permission]` 宏使用了不同的编码（WMS, MES 等）。两者没有统一。

**修复方案**（二选一）：
1. **统一到 RESOURCE_ACTION_DEFS**：修改所有 Handler 的 `require_permission` 使用标准编码
2. **扩展 RESOURCE_ACTION_DEFS**：添加 WMS、MES 等编码到定义中

推荐方案 1，因为 RESOURCE_ACTION_DEFS 的粒度更细（区分 WAREHOUSE/LOCATION/INVENTORY），更符合业务语义。

### M-1: 操作码不匹配

**根因**：部分 Handler 使用了非标准操作码 `write`，而权限系统只识别 `create`/`read`/`update`/`delete`。

**修复**：将 `write` 替换为 `create` 或 `update`（根据具体语义）。

## 7. 修复优先级建议

| 优先级 | 缺陷 | 修复工作量 |
|--------|------|------------|
| P0 | C-4: 资源编码统一 | 中（需要搜索所有 Handler，统一编码） |
| P0 | C-1: NavFilter 调试修复 | 小（定位构造函数参数问题） |
| P0 | C-2: has_permission 按钮过滤 | 小（检查各页面的 has_permission 调用） |
| P1 | C-3: Dashboard 权限检查 | 小（添加 require_permission 宏） |
| P1 | M-1: write → create/update | 小（全局搜索替换） |
| P2 | M-2: 主数据子菜单过滤 | 与 C-1 同根因 |
| P3 | m-1: 403 错误页面 | 中（需要设计统一错误页面） |

## 8. 附录

### 资源编码对照表（实际 vs 定义）

| 模块 | RESOURCE_ACTION_DEFS | Handler 实际使用 | 状态 |
|------|---------------------|------------------|------|
| 客户管理 | CUSTOMER | CUSTOMER | ✅ 匹配 |
| 产品管理 | PRODUCT | PRODUCT | ✅ 匹配 |
| 分类管理 | CATEGORY | CATEGORY | ✅ 匹配 |
| BOM | BOM | BOM | ✅ 匹配 |
| 价格 | PRICE | PRICE | ✅ 匹配 |
| 销售订单 | SALES_ORDER | SALES_ORDER | ✅ 匹配 |
| 发货 | SHIPPING | SHIPPING | ✅ 匹配 |
| 采购 | PURCHASE_ORDER | PURCHASE_ORDER | ✅ 匹配 |
| 仓库管理 | WAREHOUSE | WAREHOUSE + WMS | ❌ 不匹配 |
| 储位 | LOCATION | LOCATION + WMS | ❌ 不匹配 |
| 库存 | INVENTORY | INVENTORY + WMS | ❌ 不匹配 |
| 生产 | WORK_ORDER | MES | ❌ 不匹配 |
| 检验 | INSPECTION | MES | ❌ 不匹配 |
| 人工成本 | LABOR_COST | MES | ❌ 不匹配 |
| 成本 | COST | COST | ✅ 匹配 |
| 用户 | USER | USER | ✅ 匹配 |
| 角色 | ROLE | ROLE | ✅ 匹配 |
| 部门 | DEPARTMENT | DEPARTMENT | ✅ 匹配 |
| 财务 | FMS | FMS | ✅ 匹配 |

### 测试数据

- 种子脚本：`tests/permission/seed.sql`
- 清理脚本：`tests/permission/cleanup.sql`
- 测试用户密码：`test1234`（所有用户）
