# MES UI 对齐修复计划

**日期**：2026-06-13 | **范围**：MES 模块 7 页面 | **待修复项**：43（🔴15 / 🟡20 / 🟢8）

## 总览

| 页面 | 原型 | 实现 | 🔴 | 🟡 | 🟢 | 匹配度 | 工作量 |
|------|------|------|-----|-----|-----|--------|--------|
| MES 首页 | 04-index.html | mes_dashboard.rs | 1 | 0 | 0 | ~90% | 中(1-3h) |
| 工单列表 | 04-order-list.html | mes_order_list.rs | 4 | 1 | 2 | ~40% | 大(>3h) |
| 工单详情 | 04-order-detail.html | mes_order_detail.rs | 2 | 6 | 1 | ~25% | 大(>3h) |
| 计划详情 | 04-plan-detail.html | mes_plan_detail.rs | 3 | 6 | 1 | ~25% | 大(>3h) |
| 需求池列表 | 04-demand-pool.html | mes_demand_pool.rs | 2 | 0 | 0 | ~70% | 大(>3h) |
| 需求池创建 | 04-demand-pool-create.html | mes_demand_pool_create.rs | 2 | 2 | 0 | ~60% | 大(>3h) |
| 产品详情 | 09-product-detail.html | product_detail.rs | 1 | 5 | 4 | ~30% | 中(1-1.5d) |

**整体匹配度：~45%** | **待修复：43 项** | **预估总工作量：8–12 人天**

---

## 🔴 跨页共享基础设施（阻塞项，须先做）

以下 6 项被多个页面依赖，建议在阶段 0 统一建设，避免重复劳动。

### S1. work_centers 主数据模块（全局系统级债务）

- **现状**：`work_center_id` 外键列在 `production_plan_items`、`work_orders`、`work_order_routings` 三张表已存在，但 **无 work_centers 主数据表/Service**。id 无法解析为名称（如"A线 SMT"）。
- **影响页面**：工单列表 O3（车间列）、需求池创建（工作中心选择器）、工单详情（生产配置 section）、计划详情（信息 grid 生产中心）。
- **方案**：新建 migration `CREATE TABLE work_centers`（id/name/workshop/description/status/created_at/...）+ `abt-core/src/master_data/work_center/` 模块（model/repo/service/implt/mod，含 list 查询供 select 用）。
- **工作量**：大（>3h），全新子系统。
- **降级方案**：车间列临时显示"—"，直到主数据建成。

### S2. AuditLogService 接入 AppState

- **现状**：`AuditLogService::query_logs` 在 abt-core trait+repo **已完整实现**，但 `state.rs` **未接入**（无 `audit_log_service()` 方法）。
- **影响页面**：工单详情 #9（操作日志 Tab）、计划详情 #8（操作日志 Tab）。
- **方案**：`state.rs` 新增 `audit_log_service()` 方法（`new_audit_log_service(pool)`）。一次接入两页受益。
- **工作量**：小（<30min）。

### S3. 共享 detail_tabs 组件

- **现状**：详情页 Tab 切换无现成 Maud 组件。`wms_bin_detail.rs:152-360` 有 `detail-tabs`/`tab-panel`+`switchTab`（vanilla JS onclick）先例。
- **影响页面**：工单详情、计划详情、产品详情（三页均需 Tab 化）。
- **方案**：新建共享 `detail_tabs(active, tabs)` 组件（Maud），Tab 切换用 Surreal.js `me().on('click')` 内联（遵循 CLAUDE.md 纯前端 UI 规范）。配套 shortcut：`detail-tabs`/`detail-tab`/`tab-panel`/`tab-count`。
- **工作量**：中（1-3h），一次开发三页复用。

### S4. 产品完整度批量查询方法

- **现状**：BOM/Routing/物料模式存在性需逐产品判断，散落多处。
- **影响页面**：需求池列表（完整度指示器 + 待排程卡片）、MES 首页（数据质量卡片）。
- **方案**：abt-core 新增统一方法 `get_completeness_map(product_codes: &[String]) -> HashMap<String, CompletenessInfo{has_bom, has_routing, material_mode}>`，封装 `bom_nodes`/`bom_routings`/`products.material_consumption_mode` 查询。
- **工作量**：中（1-3h）。

### S5. 生产进度 + 来源追溯批量查询

- **现状**：work_order_routings 表有 status（进度可算），plan_item_id/sales_order_id（来源可解析），但无聚合方法。
- **影响页面**：工单列表 O1（进度列）、工单详情 #4（进度条）、工单列表 O4（来源列）、工单详情 #6（来源追溯 section）。
- **方案**：WorkOrderRepo 新增 `get_progress_map(ids)`（completed_steps/total_steps）+ `get_source_trace_map(ids)`（plan_doc/so_doc/customer_name）。
- **工作量**：中（1-3h）。

### S6. list_work_orders_by_plan 查询

- **现状**：`WorkOrder.plan_item_id` 存在，但无按计划查工单的方法。
- **影响页面**：计划详情 #7（下达结果 Tab）。
- **方案**：WorkOrderRepo 新增 `list_by_plan(plan_id)`（JOIN production_plan_items）。
- **工作量**：小-中（<1h）。

---

## 分阶段实施路线图

### 阶段 0 — 共享基础设施（~2-3 人天）

> 所有详情页 Tab 化和列表页新列的前置依赖。

| # | 任务 | 阻塞页面 | 工作量 |
|---|------|----------|--------|
| S1 | work_centers 主数据模块（migration + CRUD + list） | 工单列表/需求池创建/工单详情/计划详情 | 大 |
| S2 | AuditLogService 接入 state.rs | 工单详情/计划详情 | 小 |
| S3 | 共享 detail_tabs 组件 + CSS shortcut | 工单详情/计划详情/产品详情 | 中 |
| S4 | 产品完整度批量查询方法 | 需求池列表/MES 首页 | 中 |
| S5 | 生产进度 + 来源追溯批量查询 | 工单列表/工单详情 | 中 |
| S6 | list_work_orders_by_plan 查询 | 计划详情 | 小 |

### 阶段 1 — 高价值低风险页（~2 人天）

> MES 首页（1 项）+ 产品详情（Tab 化）+ 工单列表（列重构）。

### 阶段 2 — 详情页 Tab 化（~3 人天）

> 工单详情 + 计划详情（依赖 S1/S2/S3/S5/S6）。

### 阶段 3 — 需求池深度改造（~2-3 人天）

> 需求池列表（待排程视图 + 完整度）+ 需求池创建（工作中心 + 预览 Modal）。

---

## 逐页修复清单

### 1. MES 首页（04-index.html → mes_dashboard.rs）

**匹配度 ~90%** | 仅差数据质量卡片区域

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| D1 | 🔴 | 缺"数据质量"卡片区域（heading + 3 clickable 卡片：无 Routing / 无 BOM / 完整产品数） | 后端 dashboard service 新增 `get_data_quality_stats`（3 个相关子查询，products↔bom_routings/routings，products↔bom_nodes/boms.status=Published）；前端 stat cards 与快捷入口间插入 section-block + 3 列 grid，每卡片用 `<a href>` 包裹跳转产品/BOM 列表 | ✅ 新增 service 方法+repo 查询（表均已存在） | ✅ dq-card shortcut 或复用 stat-card | 中(1-3h) |

**涉及文件**：`abt-core/src/mes/dashboard/{model,repo,service,implt}.rs`、`abt-web/src/pages/mes_dashboard.rs`

---

### 2. 工单列表（04-order-list.html → mes_order_list.rs）

**匹配度 ~40%** | 表头列结构差异大，需整体重写为 9 列：工单编号/产品/计划数量/生产进度/排程/车间/来源追溯/状态/操作

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| O1 | 🔴 | 缺"生产进度"列（进度条 + "40% (2/5)" / "✓ 完成"） | WorkOrderRepo 新增进度聚合（COUNT(status=Completed)/COUNT(*)）；前端渲染 progress bar + 文本 | ✅ 新增聚合查询（work_order_routings 表已有） | ✅ wo-progress shortcut | 中 |
| O2 | 🟢 | 缺"排程"列（合并开始-结束为 "06-06 至 06-10"） | 纯模板：删开始/结束两列，合并为 cell-stack | ❌ | ❌（cell-stack 已有） | 小 |
| O3 | 🔴 | 缺"车间"列 | **依赖 S1 work_centers 主数据**；或降级显示"—" | ✅ 需 S1 | ❌ | 大 / 小(降级) |
| O4 | 🔴 | 缺"来源追溯"列（PP-计划号 → SO-销售单号 (客户)） | WorkOrderRepo 新增来源批量解析（LEFT JOIN plan_items/plans/sales_orders/customers） | ✅ 新增查询（表已有） | ✅ source-trace 类 | 中 |
| O5 | 🟢 | 多"创建人/创建时间"列（删） | 纯模板删除；resolve_op_names 链路可清理 | ❌ | ❌ | 小 |
| O6 | 🔴 | Tab 差异：原型 7 个（含进行中/已取消），实现 5 个（含待计划） | "草稿"改标签；"已取消"tab 补充（枚举已有）；"进行中"方案 B 从 work_order_routings 派生（推荐，无需改枚举/状态机） | ⚠️ 进行中需确认语义 | ❌ | 中(方案B) |
| O7 | 🟡 | 筛选栏缺状态 select + 日期范围 | filter-form 补 status select + 2 个 date input；OrderQueryParams 补 date_from/date_to | ⚠️ 补字段 | ❌ | 中 |

**涉及文件**：`abt-core/src/mes/work_order/{repo,model,service,implt}.rs`、`abt-core/src/mes/enums.rs`、`abt-web/src/pages/mes_order_list.rs`

**建议顺序**：O5 删列 + O2 排程（纯前端先行）→ O7 筛选栏 → O1 进度 + O4 来源（后端）→ O6 进行中（语义确认）→ O3 车间（依赖 S1 或降级）

---

### 3. 工单详情（04-order-detail.html → mes_order_detail.rs）

**匹配度 ~25%** | 大重构：Header（3 按钮 + 进度条）+ 4 Tab（工单信息/工序明细/关联单据/操作日志）

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| 1 | 🟢 | 缺"导出"按钮 | page-actions 新增 btn-default 导出（window.print 占位） | ❌ | ❌ | 小 |
| 2 | 🟡 | 缺"反达到"按钮 + 确认对话框 | **unrelease 后端已就绪**（implt.rs:302-439）；新增 OrderUnreleasePath 路由 + handler；page-actions Released 状态显示"反达到"+ hx-confirm | ❌（仅 Web 层） | ⚠️ dialog 样式 | 中 |
| 3 | 🟡 | 按钮语义：实现"取消"应改"反达到"（Released→Draft） | Released 状态显示"反达到"，取消保留给 Draft/Planned | ❌ | ❌ | 小 |
| 4 | 🟡 | 缺"生产进度"条区域 | handler 调 list_routings 计算进度；detail-header 新增 wo-progress（复用 S5） | ❌（接口已有） | ⚠️ wo-progress | 中 |
| 5 | 🟡 | 缺 4 Tab 布局 | **依赖 S3 detail_tabs 组件**；现有 info-card 移入 tab#info + 3 个 tab-panel | ❌ | ✅ detail-tabs 系列 | 中 |
| 6 | 🟡 | "工单信息"Tab 需重构为 4 section（基础数据/生产配置/来源追溯/备注） | 现有 8 字段拆分；新增生产配置 section（BOM 快照/工艺路线/工作中心/物料模式）；handler 补 lookup | ❌（字段都有） | ✅ info-section 系列 | 中 |
| 7 | 🔴 | 缺"关联单据"Tab（4 sub-tab：批次/领料/倒冲/报工） | list_by_work_order + list_routings（已有）；领料/倒冲查询需确认 | ⚠️ material_requisition 可能需补 | ✅ sub-tabs | 大 |
| 8 | 🔴 | 缺"操作日志"Tab（audit-timeline） | **依赖 S2 AuditLogService 接入**；query_logs(entity="WorkOrder")；复用 timeline shortcut | ✅ S2 | ❌（timeline 已有） | 中 |

**涉及文件**：`abt-web/src/routes/mes_order.rs`、`abt-web/src/pages/mes_order_detail.rs`、`abt-web/src/state.rs`（S2）

**后端就绪情况**：unrelease ✅ / list_routings ✅ / list_by_work_order ✅ / list_work_reports ✅ / 仅缺 AuditLogService 接入(S2)

---

### 4. 计划详情（04-plan-detail.html → mes_plan_detail.rs）

**匹配度 ~25%** | 大重构：Header（2 按钮 + 来源 + 信息 grid）+ 3 Tab（计划明细/下达结果/操作日志）+ 确认下达 Modal

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| 1 | 🟢 | 缺"导出"按钮 | 同工单详情 #1 | ❌ | ❌ | 小 |
| 2 | 🟡 | 缺"确认并下达"按钮 + 预览 Modal | pre_validate + release 已有；合并 confirm+release 为一步 + Modal 预览校验结果 | ❌ | ✅ modal 系列 | 中 |
| 3 | 🟡 | 缺"来源"追溯字段 | 创建人+时间现取；需求池 DP 来源需 plan↔demand 关联（短期降级显示销售订单来源） | ⚠️ DP 追踪需新关联 | ✅ detail-sub-row | 中 |
| 4 | 🟡 | 缺信息 Grid（4 列：日期/排产类型/生产中心/计划数量） | detail-header 新增 detail-info-grid；生产中心需 S1 | ⚠️ S1 | ✅ info-grid | 小-中 |
| 5 | 🟡 | 缺 3 Tab 布局 | **依赖 S3**；现有信息+表格移入 tab#detail + 2 panel | ❌ | ✅ plan-tabs | 中 |
| 6 | 🔴 | 表格列差异（实现 7 列 → 原型 9 列，含排程/关联/BOM工艺/完整度圆点） | 重构表格；完整度圆点用 pre_validate 的 has_routing/has_bom/material_shortages 派生 | ❌（pre_validate 已有） | ✅ completeness-dots | 大 |
| 7 | 🔴 | 缺"下达结果"Tab | **依赖 S6 list_work_orders_by_plan**；渲染 release-summary + 每工单卡片 | ✅ S6 | ✅ release-result 系列 | 大 |
| 8 | 🔴 | 缺"操作日志"Tab | **依赖 S2**；query_logs(entity="ProductionPlan") | ✅ S2 | ❌ | 中 |
| 9 | 🟡 | header 修复（display:block / 按钮去渐变改 btn-primary） | detail-header 布局确认 + 按钮样式 | ❌ | ⚠️ | 小 |
| 10 | 🟢 | release_result_banner 保留（即时反馈） | 不动 | ❌ | ❌ | 小 |

**涉及文件**：`abt-web/src/pages/mes_plan_detail.rs`、`abt-core/src/mes/production_plan/{repo,service}.rs`（S6）、`abt-web/src/state.rs`（S2）

---

### 5. 需求池列表（04-demand-pool.html → mes_demand_pool.rs）

**匹配度 ~70%** | 缺待排程视图 + 完整度指示器

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| 1 | 🔴 | 缺"待排程"视图 Tab（第 3 视图） | 新增 view=schedule 分支；复用 list_pending_demands（按交期排序）；新建 schedule_fragment 渲染（统计条 + 卡片列表，urgent/warning/normal 三色变体）；复用 urgency_hint。后端改 v_production_demands 视图 + DemandSummary 增 customer_name | ✅ migration 视图改 + struct 增字段 | ✅ schedule-* ~12 类 | 大 |
| 2 | 🔴 | 物料卡片缺 BOM/Routing/物料模式 完整度指示器 | **依赖 S4 完整度批量查询**；material_row 追加 completeness-indicators（BOM✓/✗、Routing✓/✗、倒冲/领料） | ✅ S4 或 MaterialAggSummary 增字段+SQL | ✅ ci-item ~5 类 | 中 |

**涉及文件**：`abt-core/src/mes/demand_handler/{model,repo}.rs`、`abt-core/migrations/036_create_demand_pool_views.sql`、`abt-web/src/pages/mes_demand_pool.rs`

---

### 6. 需求池创建（04-demand-pool-create.html → mes_demand_pool_create.rs）

**匹配度 ~60%** | 缺工作中心/优先级/创建并下达/预览 Modal

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| 1 | 🔴 | 缺"工作中心"选择器 | **依赖 S1 work_centers 主数据**；CreatePlanFromDemandsReq 增 default_work_center_id；handler 查 work_center list 传模板；select 首项"自动推断" | ✅ S1 + 增字段 | ❌ | 大 |
| 2 | 🟡 | 缺"优先级"选择器 | 默认排程区 4 列 grid 新增 priority select（1 高/2 普通/3 低）；CreatePlanFromDemandsReq 增 default_priority + fallback 逻辑 | ⚠️ 增字段 | ❌ | 小 |
| 3 | 🟡 | 缺"创建并下达"按钮 | handler create 后串联 release_to_work_orders（action=release）；action bar 新增第 3 按钮 | ❌（release 已有） | ❌ | 中 |
| 4 | 🔴 | 缺"预览确认"Modal | 方案 a：新增 preview_release_from_demands 预校验（UX 最佳，工作量大）；方案 b：先建草稿→pre_validate→确认 release（复用现有，UX 稍差） | ⚠️ 方案 a 需新接口 | ✅ preview-* ~6 类 | 大(a)/中(b) |

**涉及文件**：`abt-core/src/mes/demand_handler/{model,implt}.rs`、`abt-web/src/pages/mes_demand_pool_create.rs`

---

### 7. 产品详情（09-product-detail.html → product_detail.rs）

**匹配度 ~30%** | Tab 化为 5 Tab（基本信息/生产配置/BOM/库存/变更记录）

| # | 严重度 | 问题 | 修复方式 | 后端 | CSS | 工作量 |
|---|--------|------|----------|------|-----|--------|
| 1 | 🟡 | 缺 5 Tab 布局骨架 | **依赖 S3 detail_tabs**；基本信息移入 tab#info + 4 panel | ❌ | ✅ detail-tabs | 中 |
| 2 | 🔴 | 缺"生产配置"Tab（4 section） | 物料模式 = ProductMeta.material_consumption_mode（**字段已存在**）；BOM 关联 = check_product_usage；工艺 = get_bom_routing。聚合 4 个已有 service | ❌（服务全存在） | ✅ ~12 类 | 中 |
| 3 | 🟡 | 缺 BOM Tab | find_published_bom_by_product_code + get_leaf_nodes | ❌ | ❌ | 小 |
| 4 | 🟡 | 缺库存 Tab | InventoryTransactionService::query_stock(product_id) | ❌ | ❌ | 小-中 |
| 5 | 🟢 | 缺变更记录 Tab | list_price_history（已在 product_list 用过） | ❌ | ❌ | 小 |
| 6 | 🟡 | Header 缺停用/删除按钮 | 停用需 ProductService.set_status（**缺状态切换接口**）；删除已有 | ⚠️ 新增 set_status | ❌ | 中 |
| 7 | 🟢 | 基本信息获取途径 badge 样式 | acquire_channel badge | ❌ | ✅ | 小 |
| 8 | 🟡 | 基本信息缺产品分类显示 | **缺 product→category 反查**（CategoryService 仅 category→product） | ⚠️ 新增反查 | ✅ | 中 |
| 9 | 🟢 | 基本信息移除物料模式行（归生产配置） | 模板调整 | ❌ | ❌ | 小 |
| 10 | 🟢 | 归属部门恒显死代码修复 | 清理 | ❌ | ❌ | 小 |

**涉及文件**：`abt-web/src/pages/product_detail.rs`、`abt-core/src/master_data/product/service.rs`（set_status）、`abt-core/src/master_data/category/`（反查）

**后端就绪情况**：5 Tab 数据查询服务**全部已存在**，仅缺 2 个小补充（状态切换 + 分类反查）

---

## 风险与依赖

1. **work_centers 主数据（S1）是最大阻塞项**：4 个页面依赖。若不建，车间列/选择器需降级（显示"—"或"自动推断"）。
2. **AuditLogService 接入（S2）低成本高收益**：trait+repo 已实现，仅需 state.rs 一行桥接。
3. **CSS 类规范**：原型大量 inline style，移植时**必须全部提取**到 uno.config.ts shortcut 或 base.css（CLAUDE.md 禁内联）。预估需新增 40-50 个 CSS 类。
4. **"进行中"状态语义**（工单列表 O6）：需产品确认是新增枚举（改状态机，大）还是从工序派生（不改枚举，中）。推荐派生方案。
5. **预览 Modal 预校验**（需求池创建 #4）：方案 a（新建预校验）UX 最佳但工作量大；方案 b（建草稿后校验）复用现有接口。需决策。

## 后端接口就绪总览

| 页面 | 后端就绪度 | 需新增 |
|------|-----------|--------|
| MES 首页 | 数据查询需新增 | get_data_quality_stats |
| 工单列表 | 部分就绪 | 进度聚合 + 来源追溯查询 + (车间 S1) |
| 工单详情 | **大部分就绪** | 仅 S2（AuditLogService 接入） |
| 计划详情 | 部分就绪 | S2 + S6（list_work_orders_by_plan） |
| 需求池列表 | 部分就绪 | 视图改 + S4（完整度查询） |
| 需求池创建 | 部分**缺** | S1（work_centers）+ 预校验 |
| 产品详情 | **全部就绪** | 仅 set_status + 分类反查（小） |
