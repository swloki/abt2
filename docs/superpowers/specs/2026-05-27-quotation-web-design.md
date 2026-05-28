# 报价单 Web 页面设计

## 范围

在 abt-web2 中实现报价单的三个页面：列表页、创建页、详情页。复用 customers 模块的架构模式（TypedPath + HTMX + Alpine.js + Maud）。

## 不做的事

- 转销售订单（后续模块）
- 附件上传（后续迭代）
- 打印功能（后续迭代）
- 导出功能（后续迭代）

## 路由设计

| TypedPath | Method | 用途 |
|-----------|--------|------|
| `/admin/quotations` | GET | 列表页完整 HTML |
| `/admin/quotations/table` | GET | 列表表格片段（HTMX） |
| `/admin/quotations/new` | GET | 创建页完整 HTML |
| `/admin/quotations` | POST | 创建报价单 |
| `/admin/quotations/{id}` | GET | 详情页完整 HTML |
| `/admin/quotations/{id}/edit-form` | GET | 编辑弹窗表单（HTMX） |
| `/admin/quotations/{id}` | POST | 更新报价单 |
| `/admin/quotations/{id}/delete` | POST | 删除报价单 |
| `/admin/quotations/{id}/submit` | POST | 提交（Draft → Sent） |
| `/admin/quotations/{id}/accept` | POST | 接受（Sent → Accepted） |
| `/admin/quotations/{id}/reject` | POST | 拒绝（Sent → Rejected） |
| `/admin/quotations/products` | GET | 产品选择弹窗内容（HTMX） |

## 列表页

### 状态标签（Tabs）

全部 / 草稿 / 已发送 / 已接受 / 已拒绝 / 已过期。复用 `status_tabs` 组件。

### 过滤栏

- 搜索框：按报价单号、客户名称搜索
- 客户筛选下拉框

### 数据表格

列：报价单号、客户名称、联系人、状态、总金额、报价日期、有效期至、业务员、操作。

- 点击行跳转详情页
- 操作列（仅草稿）：编辑按钮（弹窗）、删除按钮（确认对话框）

### 分页

复用 `pagination` 组件。

## 创建页

### 客户信息区

- 客户选择下拉框（从 customer_service 获取列表）
- 选择后自动显示：联系人、电话、地址
- 联系人/联系电话自动填充（可手动修改）

### 报价信息区

4 列表单：
- 报价日期（date input，默认今天）
- 有效期至（date input，默认 +30 天）
- 付款条款（select）
- 交货条款（select）

### 产品明细区

- 可编辑表格，列：行号、产品编码、产品名称、规格描述、单位、数量、单价、折扣(%)、小计、删除按钮
- "添加产品行"按钮 → 打开产品选择弹窗
- 金额计算由 Alpine.js `x-data` 实时计算
- 底部汇总：合计金额、折扣总额、报价总额

### 产品选择弹窗

HTMX 请求 `/admin/quotations/products?keyword=xxx`，返回产品列表表格。选中产品后通过 Alpine.js 添加到明细数组并关闭弹窗。

### 底部操作栏

- 保存草稿 → POST 创建（status=Draft）
- 提交报价 → POST 创建（status=Sent）

### 备注区

Textarea 输入。

## 详情页

### 顶部

- 报价单号 + 状态标签（status-pill）
- 操作按钮（根据状态动态显示）：
  - 草稿：编辑、提交、删除
  - 已发送：接受、拒绝
  - 已接受/已拒绝/已过期：无操作按钮

### 基本信息卡片

grid 布局展示：客户名称、联系人、联系电话、业务员、报价日期、有效期至、付款条款、交货条款。

### 产品明细表格（只读）

列：行号、产品编码、产品名称、规格描述、单位、数量、单价、折扣、小计、交货日期。

### 金额汇总

成本合计、预估利润率、报价总额。

### 备注区

纯文本展示。

## 编辑弹窗（简化）

仅草稿状态可编辑。弹窗内编辑基本信息（付款条款、交货条款、备注），不可编辑产品明细。

## 状态按钮逻辑

| 当前状态 | 可用操作 |
|---------|---------|
| Draft | 编辑、提交、删除 |
| Sent | 接受、拒绝 |
| Accepted | 无 |
| Rejected | 无 |
| Expired | 无 |

## 后端集成

### abt-core 工厂函数

在 `abt-core/src/sales/quotation/mod.rs` 添加 `new_quotation_service(pool: PgPool) -> impl QuotationService`，参照 `new_customer_service` 模式。

### abt-web2 State

在 `state.rs` 添加 `quotation_service()` 方法。

### 侧边栏更新

将报价单路径从 `"#"` 改为 `"/admin/quotations"`。

## 文件清单

| 文件 | 操作 |
|------|------|
| `abt-core/src/sales/quotation/mod.rs` | 修改：添加工厂函数 |
| `abt-web2/src/state.rs` | 修改：添加 quotation_service |
| `abt-web2/src/routes/mod.rs` | 修改：注册 quotation 模块 |
| `abt-web2/src/routes/quotation.rs` | 新增：TypedPath 定义 + Router |
| `abt-web2/src/pages/mod.rs` | 修改：注册 quotation 模块 |
| `abt-web2/src/pages/quotation_list.rs` | 新增：列表页 |
| `abt-web2/src/pages/quotation_create.rs` | 新增：创建页 |
| `abt-web2/src/pages/quotation_detail.rs` | 新增：详情页 |
| `abt-web2/src/layout/sidebar.rs` | 修改：更新报价单路径 |
