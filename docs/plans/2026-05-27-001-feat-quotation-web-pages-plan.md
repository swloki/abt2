---
title: "feat: Add quotation web pages (list, create, detail)"
type: feat
status: active
date: 2026-05-27
origin: docs/superpowers/specs/2026-05-27-quotation-web-design.md
---

# feat: Add quotation web pages (list, create, detail)

## Summary

在 abt-web2 中实现报价单的三个页面（列表、创建、详情），完整复用 customers 模块的架构模式（TypedPath + HTMX + Alpine.js + Maud）。包括后端工厂函数集成、侧边栏导航更新、产品选择弹窗、动态明细行管理、状态流转操作。

---

## Problem Frame

报价单是销售流程的核心起点（报价 → 订单 → 发货 → 对账），但当前 abt-web2 前端只有客户管理页面。abt-core 后端已完整实现报价单的 service/repo/model，需要将其暴露为 Web 页面供业务人员使用。

---

## Requirements

- R1. 报价单列表页：状态标签过滤（全部/草稿/已发送/已接受/已拒绝/已过期）、关键词搜索、数据表格、分页
- R2. 报价单创建页：客户选择、报价信息表单、产品明细行（动态增删+金额计算）、保存草稿/提交报价
- R3. 报价单详情页：基本信息展示、产品明细只读表格、金额汇总、状态操作按钮
- R4. 编辑弹窗（简化）：仅草稿状态可编辑基本信息（付款条款、交货条款、备注）
- R5. 产品选择弹窗：HTMX 搜索加载产品列表，选中后添加到明细行
- R6. 状态流转：提交(Draft→Sent)、接受(Sent→Accepted)、拒绝(Sent→Rejected)
- R7. 侧边栏更新：报价单导航链接指向实际页面

---

## Scope Boundaries

- 不做转销售订单功能（后续 sales_order 模块）
- 不做附件上传
- 不做打印功能
- 不做导出功能
- 不做编辑页完整产品明细编辑（仅弹窗编辑基本信息）

### Deferred to Follow-Up Work

- 产品价格自动填充（需集成 ProductPriceService）：后续迭代
- 附件上传：后续迭代
- 打印/导出：后续迭代
- 转销售订单：等 sales_order 模块就绪

---

## Context & Research

### Relevant Code and Patterns

- **Customer 模块全链路** — 完整参照模式：
  - `abt-web2/src/routes/customer.rs` — TypedPath 定义 + Router 组装
  - `abt-web2/src/pages/customer_list.rs` — 列表页（status_tabs + filter-bar + data-table + pagination + modal）
  - `abt-web2/src/pages/customer_detail.rs` — 详情页（info-card + 子资源 CRUD）
  - `abt-core/src/master_data/customer/mod.rs` — 工厂函数模式
  - `abt-web2/src/state.rs` — Service 工厂方法注册
- **共享 UI 组件**：`components/tabs.rs`（status_tabs）、`components/pagination.rs`、`components/modal.rs`、`components/confirm_dialog.rs`
- **abt-core 报价单服务**：`abt-core/src/sales/quotation/service.rs` — QuotationService trait（create, find_by_id, update, submit, accept, reject, expire, list, list_items）
- **abt-core 报价单模型**：`abt-core/src/sales/quotation/model.rs` — Quotation, QuotationItem, CreateQuotationReq, QuotationQuery 等
- **abt-core 产品服务**：`abt-core/src/master_data/product/service.rs` — ProductService trait（list, get_by_ids）
- **abt-core 产品模型**：`abt-core/src/master_data/product/model.rs` — Product（product_id, product_code, pdt_name, unit, meta.specification）
- **原型设计**：`docs/ui-design/sales/quotation-list.html`、`quotation-create.html`、`quotation-detail.html`

### Institutional Learnings

- `abt-web2/CLAUDE.md` 规定了组件化三原则（绝对内聚 `hx-target="this"`、状态随身 `hx-vals`、视觉闭环 `hx-indicator`）
- `abt-web2/CLAUDE.md` 规定纯前端 UI 状态（modal 显隐）由 Alpine.js 管理，禁止通过 HTMX 向后端发请求
- `abt-web2/CLAUDE.md` 规定所有样式通过 UnoCSS 管理，禁止新增独立 CSS 文件
- `abt-web2/CLAUDE.md` 规定使用 TypedPath，禁止硬编码字符串 URL
- 项目约束：不用 `cargo run`，验证用 `cargo clippy`

---

## Key Technical Decisions

- **创建页表单提交用 JSON**：报价单创建涉及动态数组（产品明细行），标准的 `Form<T>` + `serde_urlencoded` 不支持嵌套序列结构。使用 `Json<CreateQuotationWebRequest>` 接收，前端 Alpine.js 通过 `fetch()` 提交 JSON。其他简单表单（编辑、状态变更）继续用标准 HTMX Form 提交。
- **产品选择用 HTMX 搜索弹窗**：点击"添加产品行"打开 Alpine.js modal → HTMX 请求 `/admin/quotations/products?keyword=xxx` → 渲染产品列表 → 选中后通过 `Alpine.store` 或直接调用父组件方法添加行。
- **客户信息联动用 HTMX**：选择客户后 HTMX 请求加载客户联系信息（联系人列表、地址），填充到表单字段。
- **金额计算纯前端 Alpine.js**：行小计 = 数量 × 单价 × (1 - 折扣%)，总计 = Σ小计。不依赖后端计算，提交时后端会重新校验。
- **产品模块缺工厂函数需补建**：`abt-core/src/master_data/product/mod.rs` 没有 `new_product_service()` 工厂函数（仅导出 trait 和 model），需要按 customer 模式补建。
- **编辑用弹窗而非独立页面**：仅草稿状态可编辑，且只编辑基本信息（付款条款、交货条款、备注），不做产品明细编辑。降低复杂度。

---

## Open Questions

### Resolved During Planning

- **产品选择交互**：用户选择弹窗方式（非下拉框）
- **编辑流程**：用户选择简化编辑（弹窗编辑基本信息）
- **实现范围**：全部三个页面一次完成

### Deferred to Implementation

- **产品价格自动填充**：当前不集成 ProductPriceService，用户手动输入单价。后续可加。
- **客户联系人联动细节**：CreateQuotationReq 中 contact_id 是必填 i64，需要从 customer 的联系人中选择。实现时确定是自动选主要联系人还是提供下拉选择。

---

## Output Structure

```
abt-core/src/sales/quotation/
  mod.rs                         # 修改：添加 new_quotation_service 工厂函数

abt-core/src/master_data/product/
  mod.rs                         # 修改：添加 new_product_service 工厂函数

abt-web2/src/
  state.rs                       # 修改：添加 quotation_service(), product_service()
  routes/
    mod.rs                       # 修改：注册 quotation 模块
    quotation.rs                 # 新增：TypedPath 定义 + Router
  pages/
    mod.rs                       # 修改：注册 quotation 模块
    quotation_list.rs            # 新增：列表页 + 创建处理 + 表格片段 + 编辑弹窗
    quotation_create.rs          # 新增：创建页 + 产品选择弹窗 + 客户信息联动
    quotation_detail.rs          # 新增：详情页 + 状态操作
  layout/
    sidebar.rs                   # 修改：报价单路径 "#" → "/admin/quotations"
```

---

## Implementation Units

### U1. Backend Foundation — Factory Functions & State Integration

**Goal:** 在 abt-core 中添加报价单和产品的工厂函数，在 abt-web2 AppState 中暴露服务方法。

**Requirements:** R1, R2, R3（所有页面的后端依赖）

**Dependencies:** None

**Files:**
- Modify: `abt-core/src/sales/quotation/mod.rs`
- Modify: `abt-core/src/master_data/product/mod.rs`
- Modify: `abt-web2/src/state.rs`

**Approach:**
- 报价单工厂函数：参照 `abt-core/src/master_data/customer/mod.rs` 的 `new_customer_service()` 模式。`QuotationServiceImpl::new()` 接受 7 个参数（repo, item_repo, doc_seq, state_machine, audit, event_bus, customer_svc），其中 customer_svc 通过 `new_customer_service()` 创建并包装为 `Arc<dyn CustomerService>`。
- 产品工厂函数：`ProductServiceImpl::new()` 接受 5 个参数（repo, doc_seq, audit, event_bus, state_machine），不需要跨服务依赖。
- State 方法：`quotation_service()` 和 `product_service()` 返回 `impl Trait`，与现有 `customer_service()` 签名一致。

**Patterns to follow:**
- `abt-core/src/master_data/customer/mod.rs` — 工厂函数模式（Arc 包装共享服务）
- `abt-web2/src/state.rs` — `customer_service()` 方法模式

**Test scenarios:**
- Test expectation: none — 纯连接代码，编译验证即可（`cargo clippy` 通过即正确）

**Verification:**
- `cargo clippy -p abt-core -p abt-web2` 编译通过
- `quotation_service()` 和 `product_service()` 方法可用

---

### U2. Routes, Module Registration & Sidebar

**Goal:** 创建报价单路由定义、注册所有模块、更新侧边栏导航。

**Requirements:** R7

**Dependencies:** U1

**Files:**
- Create: `abt-web2/src/routes/quotation.rs`
- Modify: `abt-web2/src/routes/mod.rs`
- Modify: `abt-web2/src/pages/mod.rs`
- Modify: `abt-web2/src/layout/sidebar.rs`
- Create: `abt-web2/src/pages/quotation_list.rs`（stub）
- Create: `abt-web2/src/pages/quotation_create.rs`（stub）
- Create: `abt-web2/src/pages/quotation_detail.rs`（stub）

**Approach:**
- TypedPath 定义所有路由（12 个），参照 `routes/customer.rs` 的命名模式
- Router 组装：按 HTTP method 注册 handler 函数
- 创建页面的 stub 文件（空 handler 返回 "TODO"），确保编译通过
- 侧边栏：将 `path: "#"` 改为 `path: "/admin/quotations"`

**Patterns to follow:**
- `abt-web2/src/routes/customer.rs` — TypedPath 定义 + Router 组装
- `abt-web2/src/layout/sidebar.rs` — NavItem 路径

**Test scenarios:**
- Test expectation: none — 路由注册和模块声明，编译验证即可

**Verification:**
- `cargo clippy -p abt-web2` 编译通过
- 侧边栏报价单链接指向 `/admin/quotations`

---

### U3. Quotation List Page

**Goal:** 实现完整的报价单列表页面，包含状态标签过滤、搜索、数据表格、分页、操作按钮。

**Requirements:** R1

**Dependencies:** U2

**Files:**
- Modify: `abt-web2/src/pages/quotation_list.rs`（替换 stub）

**Approach:**
- 整体结构参照 `customer_list.rs`：`get_quotation_list`（完整页面）+ `get_quotation_table`（HTMX 片段）
- 状态标签：6 个 Tab（全部 "" / 草稿 "1" / 已发送 "2" / 已接受 "3" / 已拒绝 "4" / 已过期 "5"），复用 `status_tabs` 组件
- 过滤栏：搜索输入框（keyword）+ 客户筛选下拉框（从 customer_service 获取列表）
- 数据表格：调用 `QuotationService::list()`，渲染行数据（报价单号 link-cell、客户名称、联系人、状态 pill、总金额 mono、日期列、操作列）
- 状态 pill 样式映射：Draft→status-draft, Sent→status-sent, Accepted→status-accepted, Rejected→status-rejected, Expired→status-expired
- 操作列（仅 Draft 状态显示）：编辑按钮（hx-get edit-form, 弹窗）、删除按钮（confirm_dialog）
- 分页：复用 `pagination` 组件

**Patterns to follow:**
- `abt-web2/src/pages/customer_list.rs` — 列表页完整模式（handler + filter + table + pagination + modal）
- `abt-web2/src/components/tabs.rs` — status_tabs 组件
- `abt-web2/src/components/confirm_dialog.rs` — 删除确认对话框

**Technical design:**

查询参数结构：
```
QuotationQueryParams {
  keyword: Option<String>,
  status: Option<i16>,      // QuotationStatus as i16
  customer_id: Option<i64>,
  page: Option<u32>,
}
```

将 QueryParams 转为 `QuotationQuery`（abt-core model）传给 service。

**Test scenarios:**
- Happy path: 无过滤参数 → 返回第一页报价单列表
- Happy path: status=1 过滤 → 仅返回草稿报价单
- Happy path: keyword 搜索 → 按单号/客户名匹配
- Edge case: 空结果 → 显示"暂无报价单数据"
- Happy path: 点击行 → 跳转详情页

**Verification:**
- `cargo clippy -p abt-web2` 编译通过
- 列表页加载显示报价单数据，状态标签可切换过滤

---

### U4. Quotation Detail Page

**Goal:** 实现报价单详情页，包含基本信息、产品明细、金额汇总、状态操作按钮。

**Requirements:** R3, R6

**Dependencies:** U2

**Files:**
- Modify: `abt-web2/src/pages/quotation_detail.rs`（替换 stub）

**Approach:**
- 调用 `QuotationService::find_by_id()` 获取报价单 + `list_items()` 获取明细
- 页面结构：
  1. 返回链接（← 返回报价单列表）
  2. 顶部：报价单号 + 状态 pill + 操作按钮（按状态动态显示）
  3. 基本信息卡片：info-grid 布局展示客户、联系人、业务员、日期、条款等
  4. 产品明细表格：只读 data-table
  5. 金额汇总：成本合计、预估利润率、报价总额
  6. 备注区
- 状态按钮逻辑：
  - Draft：显示"编辑"（弹窗）、"提交"、删除
  - Sent：显示"接受"、"拒绝"
  - 其他：无操作按钮
- 状态操作通过独立 handler（submit/accept/reject）调用 `QuotationService` 对应方法，成功后 HX-Redirect 回详情页

**Patterns to follow:**
- `abt-web2/src/pages/customer_detail.rs` — 详情页模式（handler + 组件函数 + 子资源展示）
- 原型 `docs/ui-design/sales/quotation-detail.html` — 布局样式

**Test scenarios:**
- Happy path: 草稿报价单 → 显示编辑/提交/删除按钮
- Happy path: 已发送报价单 → 显示接受/拒绝按钮
- Happy path: 已接受报价单 → 无操作按钮
- Happy path: 提交操作 → 状态变为 Sent，页面刷新显示新状态
- Error path: 对非草稿执行删除 → 后端返回错误

**Verification:**
- `cargo clippy -p abt-web2` 编译通过
- 详情页展示报价单完整信息，状态按钮按逻辑显示/隐藏

---

### U5. Quotation Create Page — Handler, Form & Line Items

**Goal:** 实现报价单创建页面，包含客户选择、报价信息表单、动态产品明细行管理、JSON 提交。

**Requirements:** R2, R5

**Dependencies:** U2

**Files:**
- Modify: `abt-web2/src/pages/quotation_create.rs`（替换 stub）
- Modify: `abt-web2/src/pages/quotation_list.rs`（添加 create_quotation handler）

**Approach:**
- 创建页 handler（`get_quotation_create`）：
  - 从 customer_service 获取客户列表（用于下拉框）
  - 从 product_service 获取产品列表（用于弹窗搜索，传给 Alpine.js 作为 JSON 数据）
  - 渲染完整页面
- 页面表单布局（参照原型 `quotation-create.html`）：
  1. 客户信息区：客户 select + 联系人/电话输入
  2. 报价信息区：日期、有效期、付款条款、交货条款
  3. 产品明细区：Alpine.js 管理的动态表格 + "添加产品行"按钮
  4. 备注区
  5. 底部操作栏：保存草稿 / 提交报价
- Alpine.js 组件（`quotationCreate()` 函数，通过 `<script>` 标签注入）：
  - `items[]` 数组管理明细行
  - `addProduct(product)` 添加行
  - `removeItem(index)` 删除行
  - 计算属性：totalAmount, totalDiscount, grandTotal
  - `submitForm(action)` 通过 fetch() POST JSON
- 创建 handler（`create_quotation`）：
  - 接收 `Json<CreateQuotationWebRequest>`
  - 转换为 `CreateQuotationReq` + `Vec<CreateQuotationItemReq>`
  - 如果 action == "submit"，创建后调用 `svc.submit()`
  - 返回 JSON `{ id, redirect }`
- 产品选择弹窗（Alpine.js modal）：
  - 点击"添加产品行"打开 modal
  - 搜索框实时过滤产品列表（Alpine.js 客户端过滤，因为产品量不大）
  - 产品列表从页面初始加载时嵌入的 JSON 数据渲染
  - 选中产品 → 调用 `addProduct()` → 关闭 modal

**Patterns to follow:**
- `abt-web2/src/pages/customer_list.rs` — handler 结构（State, Session, Form/Json）
- 原型 `docs/ui-design/sales/quotation-create.html` — 布局和表单结构
- `abt-web2/CLAUDE.md` — Alpine.js 管理 modal（x-data, x-on:click）

**Technical design:**

Web 请求结构（用于 JSON 反序列化）：
```
CreateQuotationWebRequest {
  customer_id: i64,
  contact_id: Option<i64>,
  valid_until: String,           // "2026-06-26" 格式
  payment_terms: Option<String>,
  delivery_terms: Option<String>,
  remark: Option<String>,
  action: Option<String>,        // "draft" | "submit"
  items: Vec<QuotationItemWebRequest>,
}

QuotationItemWebRequest {
  product_id: i64,
  description: Option<String>,
  quantity: String,
  unit: Option<String>,
  unit_price: String,
  discount_rate: Option<String>,
  delivery_date: Option<String>,
}
```

转换逻辑：String 字段解析为 Decimal/NaiveDate，构造 `CreateQuotationReq`。

**Test scenarios:**
- Happy path: 填写完整表单 + 保存草稿 → 创建成功，重定向到详情页
- Happy path: 填写完整表单 + 提交报价 → 创建并提交，重定向到详情页
- Error path: 未选客户 → 前端验证阻止提交
- Happy path: 添加多个产品行 → 金额自动计算正确
- Edge case: 无产品行保存草稿 → 后端处理（取决于 service 层是否允许空明细）
- Happy path: 删除产品行 → 金额重新计算

**Verification:**
- `cargo clippy -p abt-web2` 编译通过
- 创建页可以填写表单、添加产品行、提交创建

---

### U6. Edit Modal & Status Action Handlers

**Goal:** 实现草稿编辑弹窗、删除、状态变更操作。

**Requirements:** R4, R6

**Dependencies:** U3, U4

**Files:**
- Modify: `abt-web2/src/pages/quotation_list.rs`（添加 edit/delete/status handlers）

**Approach:**
- 编辑弹窗（`get_edit_quotation_form` handler）：
  - HTMX GET 请求，返回 modal 内容片段
  - 表单字段：付款条款、交货条款、备注（不包含产品明细）
  - 提交到 `update_quotation` handler
- 更新 handler（`update_quotation`）：
  - 接收 `Form<UpdateQuotationForm>`
  - 转换为 `UpdateQuotationReq`
  - 调用 `svc.update()`
  - HX-Redirect 回详情页
- 删除 handler（`delete_quotation`）：
  - 调用 `svc.delete()` （软删除，仅 Draft 可删）
  - HX-Redirect 回列表页
- 状态 handler（`submit_quotation`, `accept_quotation`, `reject_quotation`）：
  - 各自调用 `svc.submit()`, `svc.accept()`, `svc.reject()`
  - 成功后 HX-Redirect 回详情页
- 所有 handler 在路由文件中注册

**Patterns to follow:**
- `abt-web2/src/pages/customer_list.rs` — edit_form handler + update handler + delete handler 模式
- `abt-web2/src/components/confirm_dialog.rs` — 删除确认

**Test scenarios:**
- Happy path: 编辑草稿 → 修改付款条款 → 保存成功
- Error path: 编辑已提交的报价单 → 后端拒绝
- Happy path: 删除草稿 → 重定向到列表页
- Happy path: 提交草稿 → 状态变为 Sent
- Happy path: 接受已发送报价单 → 状态变为 Accepted
- Happy path: 拒绝已发送报价单 → 状态变为 Rejected

**Verification:**
- `cargo clippy -p abt-web2` 编译通过
- 草稿可以编辑、提交、删除；已发送可以接受或拒绝

---

## System-Wide Impact

- **Interaction graph:** 报价单创建页需要同时访问 QuotationService、CustomerService、ProductService 三个服务。State 层作为服务工厂，handler 通过 State 获取服务实例。
- **Error propagation:** DomainError → AppError → HTTP 响应（已有 errors.rs 映射）。状态变更失败（如对已提交报价单删除）通过 DomainError 返回前端，HTMX 显示错误 toast。
- **State lifecycle risks:** 创建页的 JSON 提交不走 HTMX 表单机制，需要手动处理 HX-Redirect。用 JSON 响应 `{ redirect }` + JS `window.location.href`。
- **API surface parity:** gRPC 层已有完整报价单 CRUD + 状态管理。Web 层直接复用 abt-core Service trait，不经过 gRPC。
- **Integration coverage:** 状态流转（submit/accept/reject）需确保与后端状态机一致——Web handler 只调用 service 对应方法，状态机逻辑由 abt-core 保证。
- **Unchanged invariants:** 现有 Customer 模块不受影响。QuotationService 的事务边界和审计日志逻辑不变。

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 创建页 JSON 提交不走 HTMX 标准流程，错误处理需自定义 | Alpine.js fetch 拦截错误响应，显示 toast 提示 |
| 产品列表嵌入页面 JSON，产品量大时影响页面大小 | 当前产品量不大可接受；后续可改为 HTMX 搜索端点 |
| contact_id 必填但用户可能不知道选哪个联系人 | 默认选主要联系人，或允许为 0 由后端处理 |
| ProductServiceImpl 构造函数需要确认签名 | 实现时读取 implt.rs 确认参数列表 |

---

## Documentation / Operational Notes

- 无数据库迁移需求（报价单表已存在）
- 无新环境变量
- 无部署特殊要求

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-27-quotation-web-design.md](docs/superpowers/specs/2026-05-27-quotation-web-design.md)
- 原型设计: `docs/ui-design/sales/quotation-list.html`, `quotation-create.html`, `quotation-detail.html`
- 后端服务: `abt-core/src/sales/quotation/`
- 参照模式: `abt-web2/src/routes/customer.rs`, `abt-web2/src/pages/customer_list.rs`, `abt-web2/src/pages/customer_detail.rs`
