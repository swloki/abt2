---
title: "feat: 主数据模块前端页面实现"
type: feat
status: active
created: 2026-05-29
scope: abt-web/src/pages/ 为主数据模块实现全部 13 个原型设计页面的 Rust/Maud/HTMX 前端
origin: "原型设计路径: C:/Users/weichen/AppData/Roaming/Open Design/namespaces/release-stable-win/data/projects/63ce2980-2f4e-45a7-9b34-8050e32135c2/09-*.html"
---

# 主数据模块前端页面实现计划

## Summary

实现主数据模块全部前端页面，对齐原型设计中 09 系列的 13 个 HTML 页面。涵盖主数据总览、产品管理（列表/新建/详情）、产品分类、物料清单 BOM（列表/新建/详情）、工艺路线（列表/新建/详情）、供应商管理（列表/新建/详情）。

## Problem Frame

原型设计已完成全部主数据相关页面的 UI 设计（13 个 HTML 页面），但 `abt-web/src/pages/` 中尚无对应实现。后端 Service 层（`abt-core/src/master_data/`）已完整：product、category、bom、bom_labor_process、supplier、routing、labor_process_dict、product_watcher、price 各子模块均已实现。

**已有页面：** `customer_list.rs` 和 `customer_detail.rs` 已存在，不需要新建。

## Scope Boundaries

### In Scope
- 13 个主数据页面的 Rust/Maud/HTMX 实现
- 对应的 TypedPath 路由定义
- 在 `mod.rs` 注册新模块
- 在 sidebar 和路由表中注册新页面
- 通过 `abt-core` Service trait 调用后端数据（禁止直接 SQL）

### Out of Scope
- 客户管理页面（已有 `customer_list.rs` / `customer_detail.rs`）
- 后端 Service 层修改（已完整实现）
- 新增 CSS 文件（使用 UnoCSS + 现有 `app.css`）
- 复杂的前端交互（BOM 树拖拽、工艺路线流程图动画等）在首期用服务端渲染 + HTMX 替代

### Deferred to Follow-Up Work
- BOM 创建页面的动态树节点拖拽排序（首期用静态表格 + HTMX 增删）
- 工艺路线详情页的流程图可视化（首期用表格展示工序步骤）
- 导出功能（Excel/CSV 导出）
- 价格管理独立页面（首期在产品详情中内嵌）

## Key Technical Decisions

### KD1: 页面组织方式 — 每个实体一个文件
每个实体的 list/create/detail 合并在一个 Rust 文件中（如 `product_list.rs`、`product_create.rs`、`product_detail.rs`），遵循现有 `customer_list.rs` 等文件的组织方式。

### KD2: 列表页创建用 Modal 还是独立页面
根据原型设计：
- 产品列表、BOM 列表、工艺路线列表、供应商列表 — 原型有 Modal 创建对话框，但实际实现用 **独立创建页面**（与现有 sales/purchase 模块一致，如 `quotation_create.rs`）
- 分类列表 — 原型有 Modal，实现中也用 Modal（分类创建字段少，适合弹窗）

### KD3: 分类管理页面 — Split View
原型用左右分栏（左树右详情）。实现中用 HTMX 驱动：左侧树点击节点 → HTMX GET 请求 → 服务端渲染右侧面板。

### KD4: BOM 创建/编辑 — 表格化编辑
原型用 inline 表格编辑 BOM 节点。实现中用 HTMX 逐行添加/删除，每次操作服务端重新渲染整个表格区域。

## Implementation Units

### U1. 主数据总览页 (md_dashboard)
**Goal:** 实现主数据模块的 Dashboard 总览页，展示统计卡片、快速入口、最近活动
**Files:**
- `abt-web/src/pages/md_dashboard.rs` (新建)
- `abt-web/src/pages/mod.rs` (添加模块声明)
- `abt-web/src/routes/` (注册路由)
**Approach:**
- 路径: `/md`
- 统计卡片: 从各 Service 的 list 接口获取 count（ProductService、BomQueryService、SupplierService、CategoryService、RoutingService）
- 快速入口: 4 个卡片链接到产品、BOM、供应商、工艺路线
- 最近活动: 展示最近创建/更新的产品、BOM 等
- 参考: `dashboard.rs` 的实现模式
**Patterns to follow:** `abt-web/src/pages/dashboard.rs`
**Test scenarios:** 页面能正常渲染，统计数字正确显示
**Verification:** `cargo check` 编译通过，浏览器访问 `/md` 可见页面

### U2. 产品列表页 (product_list)
**Goal:** 实现产品管理的列表页，包含统计卡片、状态 Tab、筛选器、分页数据表
**Files:**
- `abt-web/src/pages/product_list.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/products`
- 调用 `ProductService::list()` 获取分页数据
- 统计卡片: 总数/在用/停用/作废 — 可从 list 返回的 total + 前端 filter status 分次请求
- 状态 Tab: 用 HTMX `hx-get` 带不同 status 参数替换表格区域
- 筛选器: 搜索框 + 状态/部门/分类下拉 → HTMX GET 替换表格
- 数据表: 编码(链接)/名称/规格型号/单位/获取途径/归属部门/状态/操作(编辑/删除)
- 分页: 标准 pagination 组件
**Patterns to follow:** `abt-web/src/pages/customer_list.rs`（列表页模式）、`abt-web/src/pages/sales_order_list.rs`
**Test scenarios:** 默认加载显示全部产品，按状态筛选正确，搜索功能正常，分页切换正常
**Verification:** `cargo check` 通过

### U3. 产品创建页 (product_create)
**Goal:** 实现新建产品表单页面
**Files:**
- `abt-web/src/pages/product_create.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/products/new`
- 表单字段: 产品名称*、产品编码(自动生成)、规格型号*、计量单位*、获取途径、外部编码、归属部门、旧编码、备注
- 分类选择: 需调用 `CategoryService::get_tree()` 渲染树选择器（简化为下拉）
- POST 提交调用 `ProductService::create()`
- 成功后重定向到产品详情页
- 底部操作栏: 取消 + 保存产品
**Patterns to follow:** `abt-web/src/pages/quotation_create.rs`（创建页模式）
**Test scenarios:** 提交必填字段可成功创建，缺少必填字段显示错误，取消返回列表
**Verification:** `cargo check` 通过

### U4. 产品详情页 (product_detail)
**Goal:** 实现产品详情页，展示基本信息、分类归属、规格参数、BOM 引用
**Files:**
- `abt-web/src/pages/product_detail.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/products/:id`
- 顶部: 产品图标 + 名称 + 状态 pill + meta(编码/分类/部门/创建时间) + 操作按钮(编辑/停用/删除)
- 详情卡片网格: 基本信息、分类与归属、规格参数（从 ProductMeta 解析）
- BOM 引用表: 调用 `ProductService::check_product_usage()` 展示引用该产品的 BOM 列表
**Patterns to follow:** `abt-web/src/pages/customer_detail.rs`（详情页模式）
**Test scenarios:** 正常产品详情渲染，已删除/不存在产品返回 404，BOM 引用列表正确
**Verification:** `cargo check` 通过

### U5. 产品分类管理页 (category_list)
**Goal:** 实现产品分类管理页面，左右分栏 — 左侧树 + 右侧详情面板
**Files:**
- `abt-web/src/pages/category_list.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/categories`
- 左侧树面板: 调用 `CategoryService::get_tree()` 渲染完整分类树
- 点击树节点 → HTMX GET `/md/categories/:id/detail` → 服务端渲染右侧面板
- 右侧面板: 分类信息卡 + 子分类列表 + 关联产品表格（分页）
- 创建分类 Modal: 名称 + 上级分类选择
- 编辑/删除: 在右侧面板操作
- 添加/移除产品: 在关联产品区域操作
**Patterns to follow:** Split view 模式，参考 `reconciliation_list.rs`（分栏布局）
**Test scenarios:** 树正确渲染，点击节点加载详情，创建分类成功，删除分类成功，关联产品表格正确
**Verification:** `cargo check` 通过

### U6. BOM 列表页 (bom_list)
**Goal:** 实现 BOM 列表页，统计卡片、状态 Tab、筛选器、分页数据表
**Files:**
- `abt-web/src/pages/bom_list.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/boms`
- 调用 `BomQueryService::list()` 获取分页数据
- 统计卡片: 总数/草稿/已发布/本月新建
- Tab: 全部/草稿/已发布
- 筛选器: 搜索 + 状态 + 分类
- 数据表: BOM编码/名称/产品编码/分类/版本/状态/发布时间/创建人/操作
- 分页
**Patterns to follow:** `abt-web/src/pages/product_list.rs`（本计划的 U2）
**Test scenarios:** 默认加载、状态筛选、搜索、分页
**Verification:** `cargo check` 通过

### U7. BOM 创建/编辑页 (bom_create)
**Goal:** 实现 BOM 创建页面，包含基本信息和物料节点表格编辑
**Files:**
- `abt-web/src/pages/bom_create.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/boms/new`、`/md/boms/:id/edit`
- 基本信息区: BOM名称*、BOM分类、备注
- 物料节点表格: 每行 = 排序/物料选择/名称/规格/用量/单位/损耗率/位置/工作中心/备注/删除按钮
- 添加根节点/子节点按钮 → HTMX POST 添加空行
- 物料选择: 下拉搜索产品（调用 `ProductService::list()`）
- 保存草稿: 调用 `BomCommandService::create()` 或 `update()`
- 发布: 调用 `BomCommandService::publish()`
- 底部操作栏: 取消 + 保存草稿 + 发布
**Patterns to follow:** `abt-web/src/pages/purchase_order_create.rs`（复杂表单页模式）
**Test scenarios:** 创建空 BOM，添加物料节点，保存草稿成功，发布成功，编辑已有 BOM
**Verification:** `cargo check` 通过

### U8. BOM 详情页 (bom_detail)
**Goal:** 实现 BOM 详情页，展示基本信息、BOM 树结构、成本概要
**Files:**
- `abt-web/src/pages/bom_detail.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/boms/:id`
- 顶部: BOM 图标 + 名称 + 版本 badge + 状态 pill + meta(编码/分类/节点数/创建时间) + 操作(复制BOM/删除)
- 工作流步骤: 草稿 → 已发布（根据 status 渲染）
- 基本信息卡片: 名称/编码/分类/状态/版本/发布时间/创建人/创建时间/更新时间
- BOM 结构表格: 调用 `BomQueryService::get()` 获取 BomDetail，渲染缩进的树表格
- 成本概要: 调用 `BomCostService::get_cost_report()`，渲染物料成本表 + 人工成本 + 合计
**Patterns to follow:** `abt-web/src/pages/customer_detail.rs`
**Test scenarios:** 正常 BOM 详情、草稿状态 BOM、已发布 BOM、成本概要正确
**Verification:** `cargo check` 通过

### U9. 工艺路线列表页 (routing_list)
**Goal:** 实现工艺路线列表页
**Files:**
- `abt-web/src/pages/routing_list.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/routings`
- 调用后端获取路由列表（RoutingService 或对应的 query service）
- 统计卡片: 总数/含必检工序/关联BOM/本月新建
- Tab: 全部/已关联BOM/未关联
- 筛选器: 搜索 + 关联状态
- 数据表: 路线编码/名称/工序数量/必经工序/关联BOM数/描述/操作
**Patterns to follow:** U2（产品列表页模式）
**Test scenarios:** 默认加载、Tab 切换、搜索、分页
**Verification:** `cargo check` 通过

### U10. 工艺路线创建/编辑页 (routing_create)
**Goal:** 实现工艺路线创建页面，包含基本信息和工序步骤表格
**Files:**
- `abt-web/src/pages/routing_create.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/routings/new`、`/md/routings/:id/edit`
- 基本信息区: 路线名称*、路线编码(自动)、创建人(自动)、描述
- 工序步骤表格: 排序/工序代码(下拉选择劳务工序字典)/工序名称(自动填充)/是否必经/备注/删除
- 添加工序按钮 → HTMX 添加空行
- 工序代码下拉: 调用 `LaborProcessDictService::list()` 获取可选工序
- 底部操作栏: 取消 + 保存路线
**Patterns to follow:** `abt-web/src/pages/bom_create.rs`（本计划的 U7，表格化编辑模式）
**Test scenarios:** 创建路线、添加/删除工序、保存成功、编辑已有路线
**Verification:** `cargo check` 通过

### U11. 工艺路线详情页 (routing_detail)
**Goal:** 实现工艺路线详情页，展示基本信息、工序流程、关联 BOM
**Files:**
- `abt-web/src/pages/routing_detail.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/routings/:id`
- 顶部: 路线图标 + 名称 + 编码 + meta(工序数/必经选检/关联BOM/创建时间) + 操作(编辑/复制/删除)
- 基本信息卡片: 编码/名称/描述/创建人/创建时间/更新时间
- 工序流程: 首期用表格展示（编号/工序代码/工序名称/是否必经/备注），替代原型中的流程图可视化
- 关联 BOM 表格: 展示使用此路线的 BOM 列表
**Patterns to follow:** U8（BOM 详情页模式）
**Test scenarios:** 正常路线详情、工序列表正确、关联 BOM 列表正确
**Verification:** `cargo check` 通过

### U12. 供应商列表页 (supplier_list)
**Goal:** 实现供应商管理列表页
**Files:**
- `abt-web/src/pages/supplier_list.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/suppliers`
- 调用 `SupplierService::list()` 获取分页数据
- 统计卡片: 总数/合格/试用期/潜在
- Tab: 全部/合格/试用期/潜在/不合格/黑名单
- 筛选器: 搜索 + 类别 + 状态
- 数据表: 编码/名称/类别/联系人/电话/交货天数/状态/操作
**Patterns to follow:** U2（产品列表页模式）
**Test scenarios:** 默认加载、状态筛选、搜索、分页
**Verification:** `cargo check` 通过

### U13. 供应商创建页 (supplier_create)
**Goal:** 实现供应商创建页面，包含基本信息、联系人、银行账户
**Files:**
- `abt-web/src/pages/supplier_create.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/suppliers/new`、`/md/suppliers/:id/edit`
- 区块: 基本信息(名称*/简称/编码/类别*/税号/交货天数*/付款条件/货币类型(默认CNY, 下拉选择 CNY/JPY/USD/AUD/EUR))、联系人(姓名*/职位/电话*/邮箱)、银行账户(开户银行*/账户名称*/银行账号*/默认账户)、其他(备注)
- POST 提交: 先调 `SupplierService::create()`，然后 `add_contact()` 和 `add_bank_account()`
- 底部操作栏: 取消 + 保存供应商
**Patterns to follow:** U3（产品创建页模式）+ `purchase_order_create.rs`
**Test scenarios:** 创建成功、缺少必填字段报错、编辑已有供应商
**Verification:** `cargo check` 通过

### U14. 供应商详情页 (supplier_detail)
**Goal:** 实现供应商详情页，展示基本信息、联系人、银行账户、采购历史
**Files:**
- `abt-web/src/pages/supplier_detail.rs` (新建)
- `abt-web/src/pages/mod.rs`
**Approach:**
- 路径: `/md/suppliers/:id`
- 顶部: 供应商图标 + 名称 + 状态 pill + meta(编码/类别/创建时间) + 操作(编辑/修改状态/删除)
- 基本信息卡片: 名称/简称/编码/类别/税号/交货天数/付款条件/货币类型/状态/创建人/创建时间/更新时间
- 联系人表格: 调用 `SupplierService::list_contacts()`，支持添加/编辑/删除
- 银行账户表格: 调用 `SupplierService::list_bank_accounts()`，支持添加/编辑/删除
- 采购历史: 展示关联采购单（首期可选，需 purchase 模块支持）
- 修改状态 Modal
**Patterns to follow:** U4（产品详情页模式）+ `customer_detail.rs`（联系人子表格模式）
**Test scenarios:** 正常详情渲染、联系人增删改、银行账户增删改、修改状态
**Verification:** `cargo check` 通过

### U15. 路由注册与侧边栏更新
**Goal:** 将所有新页面注册到路由表和侧边栏导航中
**Dependencies:** U1–U14
**Files:**
- `abt-web/src/main.rs` (添加路由)
- `abt-web/src/pages/mod.rs` (添加所有模块声明)
- `abt-web/src/layout/sidebar.rs` (添加主数据侧边栏菜单项)
**Approach:**
- 在 main.rs 的 Router 中注册所有 TypedPath 路由
- 在 sidebar 中添加「主数据」菜单组: 总览/产品管理/产品分类/物料清单/工艺路线/供应商管理
- 参考现有 sidebar 菜单结构
**Patterns to follow:** 现有路由注册方式（`main.rs`）、sidebar.rs 的菜单渲染
**Test scenarios:** 侧边栏可见所有主数据菜单项，点击跳转正确
**Verification:** `cargo check` 通过，侧边栏完整渲染

## Dependency Graph

```
U15 (路由注册) ← U1..U14 (所有页面)
U4 (产品详情) ← U2 (产品列表)  # 列表中点击跳转
U7 (BOM创建) ← U6 (BOM列表)
U8 (BOM详情) ← U6
U10 (路线创建) ← U9 (路线列表)
U11 (路线详情) ← U9
U13 (供应商创建) ← U12 (供应商列表)
U14 (供应商详情) ← U12
```

## Risks

1. **Service trait 接口不匹配** — 原型设计中的某些字段/功能可能在后端 Service 中尚无对应接口。解决：实现时先确认 Service 接口，缺什么补什么。
2. **BOM 树编辑复杂度** — 原型的 inline 树编辑在 Maud + HTMX 中实现成本高。首期用扁平表格 + 缩进展示。
3. **分类树选择器** — 产品创建页需要选择分类，首期用下拉替代树选择器。

## Existing Patterns Reference

- **列表页:** `customer_list.rs` — 统计卡片 + 筛选器 + 数据表 + 分页
- **创建页:** `quotation_create.rs` — 分区表单 + 底部操作栏
- **详情页:** `customer_detail.rs` — 头部标识 + 详情卡片 + 子表格（联系人等）
- **侧边栏:** `layout/sidebar.rs` — 模块化菜单渲染
- **路由注册:** `main.rs` — `Router::new().route_with_ts(handler)`
