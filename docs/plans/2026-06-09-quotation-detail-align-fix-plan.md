# 页面对齐修复计划

**日期**：2026-06-09 | **范围**：报价单详情页 | **待修复项**：3

## 总览

| 页面 | 类型 | 原型 | 实现 | 代码层 | 浏览器 | 🔴 | 🟡 |
|------|------|------|------|--------|--------|-----|-----|
| 报价单详情 | 详情页 | quotation-detail.html | quotation_detail.rs | 3项差异 | 3项差异 | 3 | 0 |

**整体匹配度：57%** (4/7 检查项通过) | **待修复：3 项**

## 浏览器 Snapshot 对比结果

| # | 检查项 | 状态 | 原型 | 实现 |
|---|--------|------|------|------|
| 1 | D1 返回链接 | ✅ | link "返回报价单列表" | link "返回报价单列表" |
| 2 | D3 标题行 | ✅ | h1.detail-no + status-pill | h1.detail-no + status-pill |
| 3 | D3 操作按钮 | ❌ | "打印"(btn-default) + "转销售订单"(btn-primary) | 仅"提交报价" |
| 4 | D5 基本信息项 | ❌ | 8项（含联系人、联系电话、业务员） | 5项（缺少3项） |
| 5 | D6 表格列 | ✅ | 10列一致 | 10列一致 |
| 6 | D7 金额汇总 | ❌ | 3行（成本合计+预估利润+报价总额） | 1行（仅报价总额） |
| 7 | 备注区 | ✅ | info-card "备注" | info-card "备注" |

---

## 逐项修复清单

### 1. 操作按钮缺失（已接受/已发送状态）

| # | 严重度 | 检查项 | 问题描述 | 修复方式 |
|---|--------|--------|----------|----------|
| 1 | 🔴 | D3 操作按钮 | 原型已接受状态有"打印"(btn-default)和"转销售订单"(btn-primary)，实现没有对应按钮 | 在 `quotation_detail.rs:137-151` 的 `page-actions` 中，为 accepted 状态添加"打印"和"转销售订单"按钮 |

**原型片段**：
```html
<button class="btn btn-default">
  <svg>...</svg>
  打印
</button>
<button class="btn btn-primary">
  <svg>...</svg>
  转销售订单
</button>
```

**修复指引**：
- 在 `quotation_detail_page()` 中添加 `is_accepted` 判断
- accepted 状态显示"打印"(btn-default) + "转销售订单"(btn-primary)
- "打印"按钮：`window.print()` 或 HTMX 打印路由（暂用 `onclick="window.print()"`）
- "转销售订单"按钮：需确认是否有对应路由，若未实现则暂加按钮占位（disabled 或链接到创建页）
- 同时保留现有 draft→"提交报价"、sent→"接受"/"拒绝" 的按钮逻辑

**涉及文件**：`abt-web/src/pages/quotation_detail.rs`

---

### 2. 基本信息缺少联系人、联系电话、业务员

| # | 严重度 | 检查项 | 问题描述 | 修复方式 |
|---|--------|--------|----------|----------|
| 2 | 🔴 | D5 标签值对 | 原型 info-grid 有 8 项，实现仅 5 项，缺少"联系人"、"联系电话"、"业务员" | 在 handler 中查询联系人信息和业务员名称，传入模板渲染 |

**原型片段**：
```html
<div class="info-item">
  <span class="info-label">联系人</span>
  <span class="info-value">李经理</span>
</div>
<div class="info-item">
  <span class="info-label">联系电话</span>
  <span class="info-value mono">138-2345-6789</span>
</div>
<div class="info-item">
  <span class="info-label">业务员</span>
  <span class="info-value">王磊</span>
</div>
```

**修复指引**：
1. **Handler 层** (`quotation_detail.rs:36-66`)：
   - 新增查询：通过 `customer_svc.list_contacts(ctx, conn, customer_id)` 获取联系人列表
   - 从联系人列表中找到 `contact_id` 匹配的 `CustomerContact`，提取 `name` 和 `phone`
   - 新增查询：通过 `state.user_service().get_user(ctx, conn, sales_rep_id)` 获取业务员 `display_name`
   - 将联系人名称、联系电话、业务员名称传入 `quotation_detail_page()`

2. **模板函数** (`quotation_detail_page`)：
   - 新增参数：`contact_name: &str`, `contact_phone: &str`, `sales_rep_name: &str`
   - 在 info-grid 中客户名称后添加 3 个 info-item：
     - 联系人 → `info-value`
     - 联系电话 → `info-value mono`
     - 业务员 → `info-value`

3. **AppState** (`state.rs`)：确认 `user_service()` 工厂方法存在

**数据来源**：
- `Quotation.contact_id` → `CustomerService::list_contacts` → 匹配 `CustomerContact.name` + `phone`
- `Quotation.sales_rep_id` → `UserService::get_user` → `User.display_name`

**涉及文件**：`abt-web/src/pages/quotation_detail.rs`（handler + 模板）

---

### 3. 金额汇总缺少成本合计和预估利润

| # | 严重度 | 检查项 | 问题描述 | 修复方式 |
|---|--------|--------|----------|----------|
| 3 | 🔴 | D7 金额汇总 | 原型有 3 行汇总（成本合计+预估利润+报价总额），实现仅 1 行（报价总额） | 在 amount-summary 中添加成本合计和预估利润行 |

**原型片段**：
```html
<div class="amount-summary">
  <div class="amount-row">
    <span class="amount-label">成本合计</span>
    <span class="amount-value">¥ 98,360.00</span>
  </div>
  <div class="amount-row">
    <span class="amount-label">预估利润</span>
    <span class="amount-value text-success">23.5%</span>
  </div>
  <div class="amount-row">
    <span class="amount-label">报价总额</span>
    <span class="amount-value accent">¥ 128,740.00</span>
  </div>
</div>
```

**修复指引**：
- 数据模型已有 `Quotation.total_cost: Decimal` 和 `Quotation.estimated_margin: Decimal`
- 在 `quotation_detail.rs:211-218` 的 amount-summary 中，报价总额之前插入：
  - 成本合计行：`amount-label` "成本合计" + `amount-value` 显示 `q.total_cost`
  - 预估利润行：`amount-label` "预估利润" + `amount-value text-success` 显示百分比（`estimated_margin * 100`）

**涉及文件**：`abt-web/src/pages/quotation_detail.rs`

---

## 修复优先级

1. **#3 金额汇总** — 纯模板修改，数据已有，零风险，5 分钟
2. **#1 操作按钮** — 纯模板修改，需确认路由，10 分钟
3. **#2 基本信息** — 需改 handler + 模板 + 查新接口，20 分钟

**预计总工时**：35 分钟

**涉及文件汇总**：
- `abt-web/src/pages/quotation_detail.rs`（全部 3 项修复）
