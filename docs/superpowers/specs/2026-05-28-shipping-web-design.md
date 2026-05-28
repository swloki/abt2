# 发货申请页面原型对齐设计

## 背景

`/admin/shipping` 的三个页面（列表、详情、新建）需要与原型设计对齐。后端 `ShippingRequestService` 和相关 service 已就绪，前端使用已建立的模式（Axum + Maud + HTMX + Alpine.js）。

## 变更范围

### 1. 列表页 — `shipping_list.rs`

| 差距 | 修复方式 |
|------|---------|
| 缺少「新建发货申请」按钮 | page-header 添加 `btn-primary` + plus icon，链接到创建页 |
| 行操作只有「查看」 | 改为 `row-actions` 容器，含编辑（draft 状态）和删除（draft 状态）两个按钮 |
| 搜索 placeholder 缺少"客户名称" | 改为 "搜索发货单号、客户名称…" |
| 来源订单是纯文本 | 改为 `<a>` 链接，`color:var(--info)`，跳转到订单详情 |
| Status tabs 无各状态计数 | 为每个状态 tab 查询计数 |

**删除功能**：使用 confirm dialog 模式（同 sales_order_list），通过 HTMX POST 调用删除接口（对 draft 状态的发货单）。

### 2. 详情页 — `shipping_detail.rs`

| 差距 | 修复方式 |
|------|---------|
| 缺少 back-link | 添加 `a.back-link` 组件 |
| 页头布局不对 | 改为 `detail-header` 布局：左侧 doc_number + status pill + 来源订单链接；右侧操作按钮 |
| 信息卡片结构不对 | 改为 `info-card` + `info-card-title`("发货信息") |
| 信息项不对 | 改为：客户名称、收货地址、预计发货日期、承运商、物流单号、操作员 |
| 明细表列不对 | 改为：行号、产品编码、产品名称、规格描述、单位、申请数量、已发货、发货仓库 |
| 备注在 info-grid 中 | 改为独立的 `info-card`（"备注"） |

**后端数据增强**：
- 查询操作员名称（通过 operator_id 关联用户）
- 查询产品编码、名称、规格、单位
- 查询仓库名称

### 3. 新建页 — 全新页面 `shipping_create.rs`

这是全新页面，完全按原型实现。采用 Alpine.js + HTMX 表单模式。

**页面结构**：
1. back-link（返回发货申请列表）
2. page-header（"新建发货申请" + 自动保存草稿提示）
3. 客户信息区（form-section）：客户选择 → 自动填充联系人/电话/地址 → 来源订单选择器
4. 发货信息区（form-section）：预计发货日期、承运商、默认仓库、优先级、备注
5. 发货产品明细区（form-section）：行项目表格 + 添加行按钮 + 汇总栏
6. 底部操作栏：保存草稿 + 提交申请

**关键交互**：
- **客户选择**：下拉框选择客户后，HTMX 请求获取联系人/电话/地址并自动填充
- **订单选择弹窗**：Alpine.js modal，HTMX 加载该客户的已确认/部分发货订单列表，支持搜索和状态筛选，双击确认选择
- **选择订单后**：自动填入订单的产品行项目（编码、名称、规格、单位、订单数量、已发货数量），本次发货默认 = 订单数量 - 已发货
- **行项目操作**：可删除行（至少保留1行）、可手动添加空行
- **汇总**：实时计算发货项目数和发货总数量
- **提交**：`hx-post` 到后端，后端调用 `ShippingRequestService::create_from_order`，返回 `HX-Redirect`

**JavaScript 文件**：`static/shipping-create.js`，导出 `shippingForm()` Alpine.js 函数。

### 4. 基础设施变更

**`state.rs`**：添加 `shipping_service()` 和 `warehouse_service()` 工厂方法。

**`routes/shipping.rs`**：
- 添加创建页路由（GET /admin/shipping/create, POST /admin/shipping/create）
- 添加删除路由（POST /admin/shipping/{id}/delete）
- 添加 HTMX 辅助路由（获取客户联系人、获取订单列表）

**现有页面迁移**：列表页和详情页从直接 SQL 迁移到使用 `ShippingRequestService` trait。

## 数据流

### 创建流程
```
用户选择客户 → HTMX 获取联系人/地址 → 填充表单
用户打开订单弹窗 → HTMX 搜索该客户订单 → 用户选择订单 → 前端自动填充行项目
用户填写发货数量/仓库 → 点击提交 → hx-post → 后端 create_from_order → HX-Redirect 到详情页
```

### 请求结构
```rust
// POST 表单数据
struct ShippingCreateForm {
    customer_id: i64,
    order_id: i64,
    expected_ship_date: String,
    shipping_address: String,
    carrier: Option<String>,
    warehouse_id: Option<i64>,
    remark: Option<String>,
    items_json: String,  // Alpine.js 序列化的行项目
}

// items_json 结构
struct ShippingItemInput {
    order_item_id: i64,
    product_id: i64,
    warehouse_id: i64,
    requested_qty: f64,
}
```

## 不做的事

- 不修改后端 service 层逻辑
- 不修改数据库 schema
- 不实现编辑功能（后续迭代）
- 不实现优先级字段（后端模型暂无此字段，原型中的优先级下拉改为后续迭代）
