# 采购模块增强补齐设计

> 日期：2026-06-15
> 范围：补齐 P0-P2 计划中已实现后端但缺失的 Web UI 和业务逻辑集成
> 前置：P0-P2 后端代码（migrations 046-050, 5 个新模块, model/repo/service 变更）已就位且 clippy 通过

---

## 背景

P0-P2 五个计划的后端基础设施已全部实现：migrations、enums、models、repos、service traits 和 impls。但以下两类缺失：

1. **业务逻辑缺口（3 项）**：后端 service 方法存在但 confirm()/approve() 中未调用
2. **Web UI 缺口（15+ 项）**：后端 API 就绪但无路由/页面/JS

---

## 决策记录

| 决策 | 选择 | 理由 |
|---|---|---|
| 实施顺序 | 先补业务逻辑，再补 Web UI | 逻辑缺口少且独立，补完后系统行为才正确 |
| 付款计划生成策略 | 默认 100% 单期（到期日 = order_date + 30 天） | 简单可靠，后续可通过 UI 编辑 |
| Web UI 深度 | 完整页面，跟随现有 data-card/form-grid 模式 | 与现有页面一致 |
| Web UI 组织方式 | 按页面维度合并（方案 A） | 同一文件一次性改完所有计划改动，效率最高 |

---

## Phase 1：业务逻辑集成

### 1.1 confirm() 生成付款计划

**文件**：`abt-core/src/purchase/order/implt.rs`

在 `confirm()` 方法中，状态转换为 Confirmed 后（步骤 6 发布事件之后），增加：

```rust
// 8. 生成默认付款计划（100% 单期，到期日 = order_date + 30 天）
let schedule_input = vec![PaymentScheduleInput {
    due_date: order.order_date + chrono::Duration::days(30),
    payment_pct: Decimal::from(100),
    description: "全额付款".to_string(),
}];
new_payment_schedule_service(self.pool.clone())
    .generate_for_order(ctx, db, id, order.amount_total, schedule_input)
    .await?;
```

需要导入：`use crate::purchase::payment_schedule::{new_payment_schedule_service, service::PaymentScheduleService, model::PaymentScheduleInput};`

### 1.2 confirm() 自动创建供应商价格记录

**文件**：`abt-core/src/purchase/order/implt.rs`

付款计划生成后，遍历 PO items，对缺失的 supplier+product 价格记录自动创建：

```rust
// 9. 自动创建缺失的供应商价格记录
for item in &items {
    let existing = SupplierProductPriceRepo::match_best_price(
        &mut *db, order.supplier_id, item.product_id, item.quantity,
    ).await?;
    if existing.is_none() {
        SupplierProductPriceRepo::insert(&mut *db, order.supplier_id, item.product_id,
            item.unit_price, item.currency_code, ...).await?;
    }
}
```

需要在 `supplier_price/repo.rs` 中新增 `insert` 方法。

### 1.3 payment approve() 三向匹配校验

**文件**：`abt-core/src/purchase/payment/implt.rs`

在 `approve()` 方法中，状态转换之前（步骤 2 之前），增加三向匹配校验：

```rust
// 1.5 三向匹配校验：对账数量 ≤ 收货数量 + 金额一致性
if let Some(recon_id) = payment.reconciliation_id {
    let recon_items = PurchaseReconItemRepo::list_by_reconciliation_id(&mut *db, recon_id).await?;
    for item in &recon_items {
        // 找到对应 PO 明细
        let po_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, item.order_id).await?;
        let po_item = po_items.iter().find(|p| p.id == item.order_item_id)
            .ok_or_else(|| DomainError::validation(format!("订单行 {} 不存在", item.order_item_id)))?;
        // 校验：对账数量 ≤ 收货数量
        if item.received_qty > po_item.received_qty {
            return Err(DomainError::validation(format!(
                "对账数量 {} 超过收货数量 {}", item.received_qty, po_item.received_qty
            )));
        }
        // 校验：金额一致性（容差 0.5%）
        let net_qty = item.received_qty - item.returned_qty;
        let expected_amount = net_qty * item.unit_price;
        let tolerance = expected_amount * Decimal::new(5, 1000); // 0.5%
        if (item.amount - expected_amount).abs() > tolerance {
            return Err(DomainError::validation(format!(
                "对账金额 {} 与净数量×单价 {} 不匹配", item.amount, expected_amount
            )));
        }
    }
}
```

需要导入：`use crate::purchase::order::repo::PurchaseOrderItemRepo;`（已有 PurchaseReconItemRepo）。

---

## Phase 2：Web UI（按页面分批）

### 批次 1：PO 创建/编辑页

**文件**：`purchase_order_create.rs`, `purchase_order_edit.rs`, `routes/purchase_order.rs`

改动：
1. **税率下拉**：handler 加载 `tax_rate_service().list_active()` → 传入页面模板 → 每行 `<select name="tax_rate_id">` 渲染税率选项
2. **折扣输入**：每行增加 `<input name="discount_pct" type="number" step="0.01" min="0" max="100" value="0">` 列
3. **汇总区**：底部增加 `不含税金额 / 税额 / 含税总计` 三个只读字段，由 JS 实时更新
4. **P1b 付款计划编辑区**（仅 create 页）：到期日 `<input type="date">` + 百分比 `<input type="number">`，底部显示百分比合计（必须 = 100%）
5. **P2 自动取价**：选产品后发 `GET /admin/purchase/orders/auto-price?supplier_id=X&product_id=Y&quantity=Z` → 返回 JSON `{price, discount_pct, tax_rate_id}` → JS 填充行

新增路由：
- `GET /admin/purchase/orders/tax-rates` — 返回税率列表 JSON（给 JS 用）
- `GET /admin/purchase/orders/auto-price` — 自动取价

ItemWeb struct 已有 `discount_pct` 和 `tax_rate_id` 字段（已实现）。

### 批次 2：PO 详情页

**文件**：`purchase_order_detail.rs`, `routes/purchase_order.rs`

改动：
1. **P1b 付款计划展示**：加载 `payment_schedule_service().list_by_order(id)` → data-card 展示各期到期日/应付/已付/状态
2. **P1c 发票状态 badge**：`status_label()` 函数扩展或新增 `invoice_status_label()` → 显示 NoInvoice/ToInvoice/FullyInvoiced
3. **P1c 修改明细**：当 status = Confirmed/PartiallyReceived 时，显示"修改明细"按钮 → 弹出 modal（追加行/修改行/删除行）→ `POST /admin/purchase/orders/{id}/items/update`
4. **P2 审批操作**：
   - Draft 状态：显示"提交审批"按钮（调用 `submit()`）替代直接"确认"
   - PendingApproval 状态：显示"审批通过"+"退回"按钮
5. **发票信息展示**：per_billed 百分比 + invoice_status badge

新增路由：
- `POST /admin/purchase/orders/{id}/submit` — 提交审批
- `POST /admin/purchase/orders/{id}/approve` — 审批通过
- `POST /admin/purchase/orders/{id}/reject` — 退回（带 reason 参数）
- `POST /admin/purchase/orders/{id}/items/update` — 确认后修改明细

### 批次 3：PO 列表页

**文件**：`purchase_order_list.rs`, `routes/purchase_order.rs`

改动：
1. **P2 合并 PO**：列表增加 checkbox 列，选中多个 Draft 同供应商 PO → "合并选中"按钮 → `POST /admin/purchase/orders/merge`
2. **P2 状态筛选**：筛选条增加 PendingApproval 选项
3. **P1c 发票状态列**：表格增加"开票状态"列

新增路由：
- `POST /admin/purchase/orders/merge` — 合并 PO（接收 order_ids 参数）

### 批次 4：新管理页面

**文件**：3 个新 page 文件 + 3 个新 route 文件

#### 4a. `purchase_settings.rs` + `routes/purchase_settings.rs`
- 页面：data-card 表单，字段：超收容差%、超欠容差%、价格一致性开关、收货必关联 PO 开关
- 路由：`GET /admin/purchase/settings` + `POST /admin/purchase/settings`
- 调用：`purchase_settings_service().get()` / `.update()`

#### 4b. `purchase_approval_rules.rs` + `routes/purchase_approval_rules.rs`
- 页面：列表（data-table）+ 新建/编辑 modal
- 路由：`GET /admin/purchase/approval-rules` + `POST ...create` + `POST ...{id}/delete`
- 调用：`purchase_approval_service().list_rules()`（只读列表，CRUD 需在 service 中增加 create/delete 方法）

#### 4c. `supplier_price_catalog.rs` + `routes/supplier_price_catalog.rs`
- 页面：列表（按供应商/产品筛选）+ 新建/编辑 modal
- 路由：`GET /admin/purchase/supplier-prices` + `POST ...create` + `POST ...{id}/delete`
- 调用：`supplier_price_service().list_by_supplier()` / `.list_by_product()`（CRUD 需在 service 中增加 create/delete 方法）

### 批次 5：前端 JS

**文件**：`static/app.js`

新增函数：
1. **`calcPurchaseLine(rowEl)`** — 实时计算单行：subtotal = qty × price × (1 - discount/100)；tax = subtotal × taxRate/100；total = subtotal + tax。更新行内小计/税额/含税显示。
2. **`updatePurchaseSummary()`** — 遍历所有行聚合到底部汇总区（不含税/税额/含税总计）
3. **`collectPurchaseItems()`** — 替代现有 `collectItems()`，额外收集 discount_pct 和 tax_rate_id
4. 事件绑定：qty/price/discount/taxRate 变化时触发 calcPurchaseLine + updatePurchaseSummary

---

## 验收标准

### Phase 1
- [ ] confirm() 后 purchase_payment_schedules 表有 1 行 100% 记录
- [ ] confirm() 后 supplier_product_prices 表有对应供应商+产品记录
- [ ] payment approve() 时对账数量 > 收货数量返回 Validation 错误
- [ ] cargo clippy 通过

### Phase 2
- [ ] PO 创建页每行有折扣%输入和税率下拉
- [ ] 输入数量/单价/折扣/税率时实时计算小计和税额
- [ ] 底部汇总区正确显示不含税/税额/含税
- [ ] PO 确认后详情页显示付款计划
- [ ] Confirmed PO 详情页可修改明细
- [ ] 详情页显示发票状态 badge
- [ ] Draft PO 可提交审批，PendingApproval 可审批/退回
- [ ] 列表页可合并多个 Draft PO
- [ ] 采购设置页面可修改参数
- [ ] 审批规则页面 CRUD 正常
- [ ] 供应商价格目录页面 CRUD 正常
- [ ] 所有新页面 cargo clippy 通过

---

## 文件清单

### Phase 1 修改（3 个文件）
| 文件 | 改动 |
|---|---|
| `abt-core/src/purchase/order/implt.rs` | confirm() 增加付款计划生成 + 供应商价格创建 |
| `abt-core/src/purchase/payment/implt.rs` | approve() 增加三向匹配校验 |
| `abt-core/src/purchase/supplier_price/repo.rs` | 新增 insert 方法 |

### Phase 2 修改/新增
| 文件 | 改动 |
|---|---|
| `abt-web/src/pages/purchase_order_create.rs` | 税率下拉/折扣输入/汇总区/付款计划/自动取价 |
| `abt-web/src/pages/purchase_order_edit.rs` | 同上 |
| `abt-web/src/pages/purchase_order_detail.rs` | 付款计划展示/修改明细/发票状态/审批操作 |
| `abt-web/src/pages/purchase_order_list.rs` | 合并 PO/状态筛选/发票列 |
| `abt-web/src/routes/purchase_order.rs` | 新增 6 条路由 |
| `abt-web/src/pages/purchase_settings.rs` | **新建** |
| `abt-web/src/routes/purchase_settings.rs` | **新建** |
| `abt-web/src/pages/purchase_approval_rules.rs` | **新建** |
| `abt-web/src/routes/purchase_approval_rules.rs` | **新建** |
| `abt-web/src/pages/supplier_price_catalog.rs` | **新建** |
| `abt-web/src/routes/supplier_price_catalog.rs` | **新建** |
| `abt-web/src/routes/mod.rs` | 注册新路由模块 |
| `static/app.js` | calcPurchaseLine + updatePurchaseSummary + collectPurchaseItems |
| `abt-core/src/purchase/approval/service.rs` | 新增 create_rule/delete_rule trait 方法 |
| `abt-core/src/purchase/approval/implt.rs` | 实现 create_rule/delete_rule |
| `abt-core/src/purchase/approval/repo.rs` | 新增 insert/delete 方法 |
| `abt-core/src/purchase/supplier_price/service.rs` | 新增 create_price/delete_price trait 方法 |
| `abt-core/src/purchase/supplier_price/implt.rs` | 实现 create_price/delete_price |
| `abt-core/src/purchase/supplier_price/repo.rs` | 新增 insert/delete 方法 |
