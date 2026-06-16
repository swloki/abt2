# 采购管理三个页面 UI 重设计 + 接口补齐

日期：2026-06-16
Open Design 原型项目：「采购管理页面原型」(project-f7fd)，含 supplier-prices / approval-rules / purchase-settings 三页。

## 背景与痛点
`/admin/purchase/supplier-prices`、`/admin/purchase/approval-rules`、`/admin/purchase/settings` 三页"反人类"：
- 价格目录：手输/展示供应商ID、产品ID（数字）；无筛选/搜索/分页；空状态教用户改 URL 参数；`create_price` 只接 4 参数，浪费 model 大量字段。
- 审批规则：审批角色手输字符串、审批人ID手输数字；金额区间冲突不可见；只能删了重建（无 update）。
- 参数配置：缺默认税率（写死 None）；保存后整页刷新无反馈；返回链接硬编码；checkbox 可点性差；内联 style。

## 后端接口补充（接口先行）

### supplier_price 模块（改动最大）
**model.rs 新增**
- `PriceListQuery { supplier_id: Option<i64>, product_id: Option<i64>, keyword: Option<String>, currency_code: Option<String>, is_active: Option<bool> }`
- `PriceView`：`SupplierProductPrice` 全字段 + `supplier_name: String` + `supplier_code: String` + `product_code: String` + `product_name: String`（JOIN suppliers + products）
- `CreatePriceRequest` / `UpdatePriceRequest`：完整字段（supplier_id, product_id, price, currency_code, min_order_qty, discount_pct, lead_time_days, tax_rate_id, valid_from, valid_until, sequence, supplier_item_code, supplier_item_name, is_active）

**service.rs**
- `list_prices(ctx, db, filter: PriceListQuery, page: PageParams) -> Result<PaginatedResult<PriceView>>`
- 扩展 `create_price(ctx, db, req: CreatePriceRequest)`（替换原 4 参数签名；调用方同步）
- `update_price(ctx, db, id: i64, req: UpdatePriceRequest) -> Result<()>`
- `get_price(ctx, db, id: i64) -> Result<PriceView>`

**repo.rs**：list_prices / get_price 用 JOIN；create/update/get 对应 SQL（参照现有 repo 风格）。

### approval 模块
**service.rs 新增**
- `update_rule(ctx, db, id, name, min_amount, max_amount, approver_role, approver_id, sort_order)`
- `get_rule(ctx, db, id) -> Result<PurchaseApprovalRule>`
- 金额冲突检测**放前端纯逻辑**（规则按 min_amount 排序后扫描缝隙/重叠），后端不加。

### settings 模块：不改后端（model 已含 default_tax_rate_id，update 已支持）。

## 前端改造（abt-web）
硬约束：列表单端点模式、`hx-target="this"`、TypedPath、`HX-Trigger` 刷新列表、Modal、禁止内联 style（提取到 uno.config.ts/base.css）、复用现有组件类与 picker。

### supplier_price_catalog.rs 重写
- 列表页单端点 `get_supplier_prices(is_htmx)`：`filter-bar`（供应商 select / keyword `search-input` / 币种 select）+ `data-table`（供应商名 / 产品名+编码 / 单价 / 起订量 / 折扣% / 有效期 / 状态 pill / 操作✎🗑）+ `pagination`
- 新增 TypedPath：`PriceCreatePath`(POST)、`PriceEditPath`(GET 回填 Modal)、`PriceUpdatePath`(POST)、`PriceDeletePath`(POST)
- 新增/编辑 Modal：供应商 `entity_picker` + `product_picker` + 完整字段
- 提交 → `HX-Trigger:"priceUpdated"` 刷新列表 + Notyf toast（不再 HX-Redirect 整页刷新）

### purchase_approval_rules.rs 重写
- 列表页：**阶梯可视化条带**（规则按 min_amount 升序 → flex 分段、按区间宽度比例、缝隙/重叠标红）+ 状态行（连续无重叠绿 / 检测到重叠红）+ 规则表格 + 新建/编辑 Modal（角色 select、审批人 `entity_picker` 选用户）
- 冲突检测前端纯逻辑；新建/编辑用 `update_rule` / `create_rule`
- `HX-Trigger:"ruleUpdated"` 刷新

### purchase_settings.rs 优化
- 补**默认税率 select**（`TaxRateService::list_active`）
- checkbox → toggle 样式（base.css 加 `.toggle-switch`）
- 返回链接改 TypedPath（`/admin/purchase/orders` 对应的 TypedPath）
- 保存 `HX-Trigger:"settingsSaved"` + toast，去掉 HX-Redirect
- 清所有内联 `style` → uno/base.css

## docs/uml-design 同步
`02-purchase.html` 追加「采购管理页面 UI 重设计 (2026-06-16)」小节：记录 supplier_price（list_prices/PriceView/create 扩展/update_price/get_price）、approval（update_rule/get_rule）接口扩展。

## 实现总结 (2026-06-16)

### abt-core 接口扩展 ✅
- **supplier_price**：PriceListQuery / PriceView / PriceUpsertRequest 模型 + list_prices / get_price / create_price(新签名) / update_price / delete_price
- **approval**：RuleUpsertRequest 模型 + get_rule / create_rule(新签名) / update_rule / delete_rule
- `cargo clippy -p abt-core` 零新错误

### abt-web 三页重写 ✅
- **supplier_price_catalog.rs**：单端点列表 + filter-bar(关键词/币种/状态) + 分页 + Modal 新增/编辑(全部14字段) + HX-Trigger 事件刷新
- **purchase_approval_rules.rs**：阶梯可视化 + 表格 + Modal 新增/编辑 + HX-Trigger 事件刷新
- **purchase_settings.rs**：税率 select + TypedPath 返回链接 + 移除内联样式 + 保留 checkbox
- `cargo clippy -p abt-web` 零新错误

### 待后续迭代
- supplier/product entity_picker（目前表单用数字输入框）
- 审批规则冲突检测（规格文档记录为前端逻辑）
- Toggle switch 样式（仍用 checkbox）
- settings 页 toast 反馈（仍用 HX-Redirect）
- `docs/uml-design/02-purchase.html` 设计文档同步（需用户确认后更新）

## 验证
`cargo clippy`（主要）+ `cargo build`。服务已在运行，不用 cargo run。
