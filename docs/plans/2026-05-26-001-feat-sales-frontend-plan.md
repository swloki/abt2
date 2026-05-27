---
title: "feat: Implement Sales Module Frontend (abt-web)"
type: feat
status: active
date: 2026-05-26
origin: docs/ui-design/01-sales-ui.md
---

# feat: Implement Sales Module Frontend (abt-web)

## Summary

基于 `docs/ui-design/01-sales-ui.md` UI 设计文档，在 `abt-web` 前端实现完整的销售模块 5 个子模块（报价单、销售订单、发货申请、销售退货、月对账单），包含列表页、详情页、表单抽屉、状态流转操作、关联单据时间线、权限控制。后端已全部实现，前端目前完全空白。

---

## Problem Frame

ABT 系统后端已完成 5 个销售子模块的 gRPC 服务实现（proto 定义 + handler + service），但前端没有对应的页面、组件或 Actions。销售人员无法通过 Web 界面操作报价、订单、发货、退货和对账流程。

---

## Requirements

- R1. 实现 Quotation 报价单前端：列表页（筛选/搜索/分页）+ 详情页（基本信息/明细行/状态流转/关联单据）
- R2. 实现 SalesOrder 销售订单前端：含"从报价单创建"流程、发货完成度展示
- R3. 实现 ShippingRequest 发货申请前端：含侧边抽屉向导式创建、OQC 质检状态提示
- R4. 实现 SalesReturn 销售退货前端：含处理方式(Restock/Scrap/Rework)汇总展示
- R5. 实现 Reconciliation 月对账单前端：含自动聚合、批量确认、重新生成
- R6. 注册 gRPC 客户端、类型定义、权限码、菜单项、路由权限
- R7. 所有页面遵循现有 Astro SSR + Svelte 5 模式
- R8. 所有页面集成 PermissionGuard 权限控制
- R9. 响应式设计（桌面端表格 + 移动端卡片）

---

## Scope Boundaries

### In Scope

- 5 个销售子模块的全部前端页面和组件
- gRPC 客户端注册、TypeScript 类型定义
- 菜单导航、面包屑、路由权限注册
- 状态流转操作（按钮可见性按状态动态控制）
- 关联单据跳转

### Out of Scope

- 后端 gRPC 服务修改（已实现完成）
- Proto 定义修改
- UI 组件库扩展（StatusFlowCard 等高级组件留待后续迭代）
- 移动端专用优化（仅确保不破坏现有响应式模式）
- 表单暂存 localStorage、并发冲突对比差异（留待后续优化）

---

## Context & Research

### Relevant Code and Patterns

- `abt-web/src/pages/admin/term/index.astro` — 列表页 SSR 数据获取模式
- `abt-web/src/components/admin/TermList.svelte` — 列表组件模式（Drawer 创建/编辑、DataTable、筛选）
- `abt-web/src/actions/term.ts` — Astro Actions 模式（defineAction + zod + gRPC 调用）
- `abt-web/src/lib/grpc-client.ts` — gRPC 客户端注册模式
- `abt-web/src/lib/menu-items.ts` — 侧边栏菜单注册
- `abt-web/src/lib/route-permissions.ts` — 路由权限映射
- `abt-web/src/lib/permission-codes.ts` — 权限码（从 proto Resource enum 自动导出）
- `abt-web/src/types/api.ts` — 前端领域类型定义模式
- `abt-web/src/components/ui/DataTable.svelte` — 通用数据表格
- `abt-web/src/components/ui/Drawer.svelte` — 侧边抽屉
- `abt-web/src/components/ui/PermissionGuard.svelte` — 权限守卫

### Proto Services (Backend Ready)

- `proto/abt/v1/quotation.proto` — QuotationService
- `proto/abt/v1/sales_order.proto` — SalesOrderService
- `proto/abt/v1/shipping.proto` — ShippingService
- `proto/abt/v1/sales_return.proto` — SalesReturnService
- `proto/abt/v1/reconciliation.proto` — ReconciliationService

### Institutional Learnings

- 所有销售子模块后端已实现，gRPC endpoints 可用
- 前端 gRPC 错误处理：`grpc-error.ts` 提取 `BusinessErrorDetails`，支持字段级验证错误
- BigInt ↔ Number 转换是必须的模式
- Drawer 模式（非路由模式）是创建/编辑表单的首选模式

---

## Key Technical Decisions

- **Drawer 模式优先**：创建/编辑表单使用 Drawer 侧边抽屉，不使用独立路由页面（遵循 term 模块先例）
- **每个子模块一个 Svelte 列表组件**：每个子模块（QuotationList、SalesOrderList 等）独立管理自己的状态、筛选、Drawer
- **详情页使用独立路由**：`/admin/sales/quotation/[id].astro`，因为详情页信息量大，Drawer 不适合
- **状态按钮可见性在前端实现**：根据当前 status 枚举值条件渲染操作按钮，无需等后端 available_actions
- **类型定义集中管理**：销售相关类型统一在 `src/types/sales.ts` 中定义
- **Actions 按子模块拆分**：每个子模块一个 Action 文件，在 `src/actions/index.ts` 统一注册
- **Quotation 作为模板先实现**：先完成 Quotation 建立模式，其余模块复制并调整

---

## Output Structure

```
abt-web/src/
├── types/sales.ts                              # 销售模块类型定义
├── lib/grpc-client.ts                          # 新增 5 个 gRPC 客户端
├── lib/menu-items.ts                           # 新增销售菜单组
├── lib/route-permissions.ts                    # 新增销售路由权限
├── actions/
│   ├── quotation.ts                            # 报价单 Actions
│   ├── sales-order.ts                          # 销售订单 Actions
│   ├── shipping-request.ts                     # 发货申请 Actions
│   ├── sales-return.ts                         # 销售退货 Actions
│   ├── reconciliation.ts                       # 月对账单 Actions
│   └── index.ts                                # 注册新 Actions
├── pages/admin/sales/
│   ├── quotation/
│   │   ├── index.astro                         # 列表页
│   │   └── [id].astro                          # 详情页
│   ├── order/
│   │   ├── index.astro
│   │   └── [id].astro
│   ├── shipping/
│   │   ├── index.astro
│   │   └── [id].astro
│   ├── return/
│   │   ├── index.astro
│   │   └── [id].astro
│   └── reconciliation/
│       ├── index.astro
│       └── [id].astro
└── components/admin/sales/
    ├── QuotationList.svelte                    # 报价单列表 + Drawer 表单
    ├── QuotationDetail.svelte                  # 报价单详情
    ├── SalesOrderList.svelte
    ├── SalesOrderDetail.svelte
    ├── ShippingRequestList.svelte
    ├── ShippingRequestDetail.svelte
    ├── SalesReturnList.svelte
    ├── SalesReturnDetail.svelte
    ├── ReconciliationList.svelte
    └── ReconciliationDetail.svelte
```

---

## Implementation Units

### U1. 基础设施（gRPC 客户端 + 类型 + 菜单 + 权限）

**Goal:** 注册销售模块的前端基础设施，使后续页面开发可以引用 gRPC 服务、类型定义和权限控制

**Requirements:** R6, R8

**Dependencies:** None

**Files:**
- Modify: `abt-web/src/lib/grpc-client.ts`
- Create: `abt-web/src/types/sales.ts`
- Modify: `abt-web/src/lib/menu-items.ts`
- Modify: `abt-web/src/lib/route-permissions.ts`
- Modify: `abt-web/src/actions/index.ts`

**Approach:**
- 在 `grpc-client.ts` 中导入 5 个 proto 服务并 `createClient`
- 在 `types/sales.ts` 中定义前端类型（Quotation/QuotationItem/QuotationStatus 等），包含 proto → 前端的映射辅助函数
- 在 `menu-items.ts` 中添加 "销售管理" 菜单组（含 5 个子项），使用 PermissionGuard 控制可见性
- 在 `route-permissions.ts` 中注册销售路由的权限映射

**Patterns to follow:**
- `abt-web/src/lib/grpc-client.ts` — 现有服务注册模式
- `abt-web/src/types/api.ts` — 类型定义模式
- `abt-web/src/lib/menu-items.ts` — 菜单项格式

**Test scenarios:**
- Test expectation: none — 基础设施注册，通过 typecheck 验证

**Verification:**
- `npm run typecheck` 通过
- 菜单项在侧边栏可见（需要后端权限种子数据中已添加销售相关资源）

---

### U2. Quotation 报价单（模板模块）

**Goal:** 实现完整的报价单前端，作为后续模块的模板：列表页（筛选/搜索/分页/批量操作）+ Drawer 创建/编辑 + 详情页（状态流转 + 明细行 + 关联单据）

**Requirements:** R1, R7, R8, R9

**Dependencies:** U1

**Files:**
- Create: `abt-web/src/pages/admin/sales/quotation/index.astro`
- Create: `abt-web/src/pages/admin/sales/quotation/[id].astro`
- Create: `abt-web/src/components/admin/sales/QuotationList.svelte`
- Create: `abt-web/src/components/admin/sales/QuotationDetail.svelte`
- Create: `abt-web/src/actions/quotation.ts`

**Approach:**

**列表页 (`quotation/index.astro` + `QuotationList.svelte`)**：
- SSR: `tryGrpcCall` 调用 `quotationService.listQuotations` 获取初始数据
- 客户端: DataTable 展示列表，支持客户/状态/日期范围/销售员筛选
- Drawer 表单: 创建/编辑报价单，含明细行增删、产品下拉联动成本/价格、实时金额计算
- 操作: 查看、编辑（仅 Draft）、删除（仅 Draft）、复制新建、状态流转按钮

**详情页 (`quotation/[id].astro` + `QuotationDetail.svelte`)**：
- SSR: `tryGrpcCall` 调用 `quotationService.getQuotation` 获取详情
- 展示: 基本信息双栏布局 + 明细行表格 + 金额摘要 + 状态徽章 + 审计信息
- 操作: 状态流转按钮（根据当前 status 动态显示）
- 关联: 显示派生的销售订单列表（如有）

**Actions (`quotation.ts`)**：
- `list` — 列表查询（含筛选参数）
- `create` — 创建报价单
- `update` — 更新报价单（仅 Draft）
- `delete` — 软删除（仅 Draft）
- `submit` — 提交 Draft → Sent
- `accept` — 接受 Sent → Accepted
- `reject` — 拒绝 Sent → Rejected
- `expire` — 过期 Sent → Expired

**Patterns to follow:**
- `abt-web/src/pages/admin/term/index.astro` — SSR 列表页模式
- `abt-web/src/components/admin/TermList.svelte` — 列表 + Drawer 模式
- `abt-web/src/actions/term.ts` — Actions 定义模式

**Test scenarios:**
- Happy path: 打开报价单列表页，显示数据表格和筛选栏
- Happy path: 点击新建，Drawer 打开表单，填写并保存，列表刷新显示新记录
- Happy path: 点击编辑 Draft 记录，修改后保存
- Happy path: 提交 Draft → Sent，状态徽章变为蓝色
- Happy path: 接受 Sent → Accepted，状态徽章变为绿色
- Happy path: 详情页展示明细行和状态流转按钮
- Edge case: 非 Draft 状态不显示编辑/删除按钮
- Error path: 创建时未填必填字段，显示验证错误
- Error path: gRPC BusinessRule 错误显示为 Toast 提示

**Verification:**
- `npm run typecheck` 通过
- 列表页加载正常，筛选和分页工作
- 创建/编辑 Drawer 表单提交正常
- 状态流转按钮按状态动态显示/隐藏
- 详情页展示完整数据

---

### U3. SalesOrder 销售订单

**Goal:** 实现销售订单前端，含"从报价单创建"流程、发货完成度展示、关联报价单跳转

**Requirements:** R2, R7, R8

**Dependencies:** U1, U2

**Files:**
- Create: `abt-web/src/pages/admin/sales/order/index.astro`
- Create: `abt-web/src/pages/admin/sales/order/[id].astro`
- Create: `abt-web/src/components/admin/sales/SalesOrderList.svelte`
- Create: `abt-web/src/components/admin/sales/SalesOrderDetail.svelte`
- Create: `abt-web/src/actions/sales-order.ts`

**Approach:**
- 复用 U2 的页面结构和组件模式，调整为 SalesOrder 的字段和状态
- **"从报价单创建"**：新增按钮，点击后弹出报价单选择 Drawer（仅显示 Accepted 状态），选择后自动填充客户/联系人/明细行
- **发货完成度**：明细行表格中 shipped_qty/returned_qty 列，详情页顶部显示完成度进度条
- **关联时间线**：详情页展示来源报价单 + 下游发货申请列表（可点击跳转）
- 状态按钮：确认（触发库存预留提示）、开始生产、完成（需发货完成度100%）、取消

**Patterns to follow:**
- U2 Quotation 的全部模式
- 关联单据跳转使用 `<a href="/admin/sales/shipping/{id}">` 链接

**Test scenarios:**
- Happy path: 从 Accepted 报价单创建销售订单，自动带入数据
- Happy path: 确认订单 → 状态变为 Confirmed
- Happy path: 详情页显示发货完成度和关联发货申请
- Edge case: 完成按钮在发货未达 100% 时不可用
- Error path: 从非 Accepted 报价单创建 → 提示错误

**Verification:**
- `npm run typecheck` 通过
- 列表/详情/表单功能正常
- 从报价单创建流程端到端工作
- 关联单据跳转正确

---

### U4. ShippingRequest 发货申请

**Goal:** 实现发货申请前端，含从订单创建的侧边抽屉向导、OQC 质检状态提示、发货量校验

**Requirements:** R3, R7, R8

**Dependencies:** U1, U3

**Files:**
- Create: `abt-web/src/pages/admin/sales/shipping/index.astro`
- Create: `abt-web/src/pages/admin/sales/shipping/[id].astro`
- Create: `abt-web/src/components/admin/sales/ShippingRequestList.svelte`
- Create: `abt-web/src/components/admin/sales/ShippingRequestDetail.svelte`
- Create: `abt-web/src/actions/shipping-request.ts`

**Approach:**
- **创建流程**：点击"从订单创建"→ 侧边抽屉打开 → 选择订单 → 展示待发货行 → 填发货数量+仓库 → 填物流信息 → 保存
- **发货量校验**：填写的发货数量不能超过 `order_qty - shipped_qty`
- **OQC 提示**：确认按钮旁显示质检状态，未通过时红色警告
- **关联**：详情页展示来源销售订单 + 逆向退货（如有）
- 状态按钮：确认（OQC 检查）、拣货、发货、取消

**Patterns to follow:**
- U2/U3 的列表/详情模式
- Drawer 向导参考 `abt-web/src/components/ui/Drawer.svelte` 多步模式

**Test scenarios:**
- Happy path: 从 Confirmed+ 订单创建发货申请，选择行、填数量、保存
- Happy path: 发货后订单 shipped_qty 自动更新
- Edge case: 发货量超过剩余可发量 → 前端校验阻止
- Edge case: OQC 未通过时确认按钮不可用
- Error path: 选择未确认订单 → 提示错误

**Verification:**
- `npm run typecheck` 通过
- 创建向导流程完整
- 发货量校验工作
- 详情页数据展示正确

---

### U5. SalesReturn 销售退货

**Goal:** 实现销售退货前端，含从发货单创建、处理方式(Restock/Scrap/Rework)选择和汇总、可退量校验

**Requirements:** R4, R7, R8

**Dependencies:** U1, U4

**Files:**
- Create: `abt-web/src/pages/admin/sales/return/index.astro`
- Create: `abt-web/src/pages/admin/sales/return/[id].astro`
- Create: `abt-web/src/components/admin/sales/SalesReturnList.svelte`
- Create: `abt-web/src/components/admin/sales/SalesReturnDetail.svelte`
- Create: `abt-web/src/actions/sales-return.ts`

**Approach:**
- **创建流程**：选择已 Shipped 的发货单 → 展示已发货行 + 可退量 → 填退货数量 + 处理方式 → 保存
- **处理方式汇总**：明细表格下方按 Restock/Scrap/Rework 统计数量
- **可退量校验**：退货量不能超过 `shipped_qty - returned_qty`
- **关联**：详情页展示来源发货申请 + 来源订单
- 状态按钮：审批、收货、检验、完成、驳回、取消

**Patterns to follow:**
- U2/U3/U4 的列表/详情/Drawer 模式

**Test scenarios:**
- Happy path: 从 Shipped 发货单创建退货，选择产品和处理方式
- Happy path: 审批 → 收货 → 检验 → 完成，状态依次流转
- Edge case: 退货量超过可退量 → 前端校验阻止
- Edge case: 处理方式汇总正确统计
- Error path: 选择未发货的单据 → 提示错误

**Verification:**
- `npm run typecheck` 通过
- 创建流程完整
- 处理方式汇总正确展示
- 状态流转按钮正确

---

### U6. Reconciliation 月对账单

**Goal:** 实现月对账单前端，含自动聚合发货明细、批量确认、确认进度展示、重新生成

**Requirements:** R5, R7, R8

**Dependencies:** U1, U4

**Files:**
- Create: `abt-web/src/pages/admin/sales/reconciliation/index.astro`
- Create: `abt-web/src/pages/admin/sales/reconciliation/[id].astro`
- Create: `abt-web/src/components/admin/sales/ReconciliationList.svelte`
- Create: `abt-web/src/components/admin/sales/ReconciliationDetail.svelte`
- Create: `abt-web/src/actions/reconciliation.ts`

**Approach:**
- **创建流程**：选择客户 + 期间 → 自动聚合发货明细 → 保存草稿
- **批量确认**：明细表格支持勾选多行 → 批量确认按钮
- **确认进度**：顶部显示 "已确认行/总行数" 进度条
- **重新生成**：Draft 状态下可点击重新聚合（保留手动修改行）
- **逐行备注**：每行可展开输入 remark
- 状态按钮：发送客户、确认、提出异议、打回重做、差异结算、结算核销

**Patterns to follow:**
- U2/U3/U4 的列表/详情模式
- 批量勾选参考 `DataTable.svelte` 的批量操作功能

**Test scenarios:**
- Happy path: 选择客户+期间创建对账单，自动聚合发货明细
- Happy path: 勾选多行批量确认
- Happy path: 确认进度条实时更新
- Happy path: 发送客户 → 客户确认 → 结算，完整流程
- Edge case: 重复创建同一客户+期间 → 提示已存在
- Edge case: 重新生成保留已修改行
- Error path: 未全部确认时点击确认 → 提示错误

**Verification:**
- `npm run typecheck` 通过
- 创建聚合流程正确
- 批量确认工作
- 状态流转完整

---

## Open Questions

### Resolved During Planning

- 页面模式：Drawer 创建/编辑 + 独立路由详情页（遵循 term 模块先例）
- 类型管理：集中到 `types/sales.ts`（遵循 api.ts 先例）
- Actions 拆分：每个子模块一个 Action 文件

### Deferred to Implementation

- 前端权限资源码：需要在后端 permission seed data 中添加销售相关的 Resource 枚举值（SALES_QUOTATION、SALES_ORDER、SHIPPING_REQUEST、SALES_RETURN、RECONCILIATION 各需 READ/WRITE 权限）
- Proto 生成的 TypeScript 类型包名：需要确认 Buf schema registry 中是否已发布销售相关 proto（当前检查 `@buf/xweichen_abt.bufbuild_es` 包）
- 高级 UI 组件（StatusFlowCard Stepper、DocumentHealthCard、BOM Popover 钻取）：留待基础功能完成后再迭代
- 移动端卡片视图：确保不破坏现有响应式模式即可，不做专门的移动端优化
