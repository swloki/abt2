# 主数据模块测试问题清单

**测试日期**: 2026-06-10
**测试层级**: Full
**测试范围**: 主数据模块 25 个页面（Product, BOM, Customer, Supplier, Category, Price History, Routing, Labor Process Dict, WMS Warehouse, WMS Bin）

## 测试总览

| 页面组 | 页面数 | 通过 | 问题数 |
|--------|--------|------|--------|
| 产品 Product | 3 | 3 | 0 |
| 物料清单 BOM | 4 | 4 | 1 (P3 CSS) |
| 客户 Customer | 2 | 2 | 0 |
| 供应商 Supplier | 4 | 4 | 0（#7 确认为设计意图） |
| 分类 Category | 1 | 1 | 0 |
| 价格历史 Price History | 1 | 1 | 0 |
| 工艺路线 Routing | 3 | 3 | 0（全部已修复） |
| 工序字典 Process Dict | 1 | 1 | 0 |
| 仓库 Warehouse | 3 | 3 | 0（全部已修复） |
| 库位 Bin | 3 | 3 | 0（全部已修复） |

## 问题列表

| # | 页面 | 测试项 | 问题描述 | 涉及文件 | 优先级 | 状态 |
|---|------|--------|---------|----------|--------|------|
| 1 | 工艺路线列表 | 搜索功能 | 搜索 keyword 时触发 500 错误。`repo.rs` 中 `param_idx` 递增时机错误导致 SQL 参数占位符不连续 | `abt-core/src/master_data/routing/repo.rs` | P1 | ✅ 已修复 |
| 2 | 工艺路线详情 | 工序名称显示 | 工序流程表格"工序名称"列显示 `process_code` 而非实际工序名称 | `routing/model.rs`, `routing/repo.rs`, `routing_detail.rs` | P2 | ✅ 已修复 |
| 3 | 库位详情 | 页面加载 | 无效 ID 访问返回 404，无标准错误页面 | `abt-web/src/pages/wms_bin_detail.rs` | P2 | ✅ 已修复 |
| 4 | 工艺路线详情 | 创建人显示 | "创建人"显示 "ID: xxx" 而非实际用户名 | `abt-web/src/pages/routing_detail.rs` | P3 | ✅ 已修复 |
| 5 | 仓库列表 | 库区数/储位数 | "库区数"和"储位数"列硬编码显示"—"，需添加子查询 | `warehouse/model.rs`, `warehouse/repo.rs`, `wms_warehouse_list.rs` | P3 | ✅ 已修复 |
| 6 | 仓库列表 | 管理员列 | "管理员"列硬编码显示"—"，需关联用户表 | `wms_warehouse_list.rs` | P3 | ✅ 已修复 |
| 7 | 供应商编辑 | 联系人/账户 | 编辑页缺少联系人和银行账户编辑区域 | — | P3 | ✅ 设计意图（详情页独立管理） |
| 8 | BOM 列表 | 分类下拉同步 | URL 参数 `category_name=电源` 过滤正确但分类下拉未同步选中 | `bom_list.rs` | P3 | ✅ 验证通过（实际已正常工作） |
| 9 | BOM 列表 | CSS 类名 | CSS 类名为 `customer-list-panel`，语义不正确 | `bom_list.rs` | P3 | ✅ 已修复 |

## 修复记录

| # | 修复方案 | 同类排查 |
|---|---------|---------|
| 1 | `repo.rs` query 方法：将 `param_idx += 1` 移到 `conditions.push()` **之前**，确保参数从 `$1` 起连续编号 | 检查其他 repo 动态查询的 param_idx 逻辑 |
| 2 | model.rs 添加 `process_name: Option<String>` + `#[sqlx(default)]`；repo.rs `find_steps` 改为 LEFT JOIN `labor_process_dicts`；detail 页模板使用 `process_name` fallback `process_code` | — |
| 3 | handler 中 match `get_bin_with_warehouse` 结果，NotFound 时渲染 `error_page("储位未找到", ...)` 并返回完整 admin_page 布局 | 其他详情页可参考此模式 |
| 4 | handler 调用 `UserService::get_users_by_ids` 解析 operator_id 为用户名，传入模板 | 参考 bom_list 的 `resolve_creator_names` 模式 |
| 5 | model.rs 添加 `zone_count: i64` + `bin_count: i64`（`#[sqlx(default)]`）；repo `list` data_sql 添加子查询统计 zones/bins 数量 | — |
| 6 | handler 收集 `manager_id` 列表，调用 `UserService::get_users_by_ids` 解析为用户名 Map，传入模板渲染 | 与 #4 同模式 |
| 7 | 联系人和银行账户在详情页通过 modal 独立管理（添加/删除），编辑页只负责基本信息，属于设计意图 | — |
| 8 | `resolve_category_name` 逻辑正确：`ILIKE '%name%'` 查询 → 取首条 → 设置 `category_id` → 下拉 `selected` 属性正常。实测 `?category_name=电源` 下拉正确选中"电源" | — |
| 9 | `customer-list-panel` → `bom-list-panel`，全部引用点（class、hx-target、status_tabs）同步替换 | 检查其他页面是否有类似语义不当的类名 |

## 回归验证结果（第二轮）

| 验证项 | 结果 |
|--------|------|
| 仓库列表：库区数显示实际数字 | ✅ 显示 "1", "2", "3" 等，非硬编码 "—" |
| 仓库列表：储位数显示实际数字 | ✅ 显示 "0", "1", "2" 等 |
| 仓库列表：管理员列（无 manager_id 的仓库） | ✅ 显示 "—"（数据层面无管理员） |
| 库位详情 404：无效 ID | ✅ 显示友好错误页面"储位未找到"，有返回首页链接 |
| BOM 列表：CSS 类名 | ✅ 使用 `bom-list-panel` |
| BOM 列表：分类下拉同步 | ✅ `?category_name=电源` 下拉正确选中"电源" |
| 工艺路线搜索（回归） | ✅ 无 500 |
| 工艺路线详情工序名称（回归） | ✅ 显示名称而非代码 |
