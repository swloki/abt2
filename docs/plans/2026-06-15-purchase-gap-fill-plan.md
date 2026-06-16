# 采购模块增强补齐实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 P0-P2 计划中缺失的业务逻辑集成（3 项）和 Web UI（5 批次），使全部验收标准达标。

**Architecture:** Phase 1 修改 3 个 abt-core 文件补齐 confirm()/approve() 业务逻辑。Phase 2 按页面维度修改 abt-web 文件，一次性合并同一文件涉及的所有计划改动。验证方式为 cargo clippy。

**Tech Stack:** Rust (axum + maud + HTMX), cargo clippy 验证

---

## Phase 1：业务逻辑集成

### Task 1: confirm() 生成付款计划 + 自动创建供应商价格

**Files:**
- Modify: `abt-core/src/purchase/order/implt.rs`
- Modify: `abt-core/src/purchase/supplier_price/repo.rs`

- [ ] **Step 1: 在 supplier_price/repo.rs 新增 insert 方法**

在 `SupplierProductPriceRepo` impl 块末尾（`list_by_product` 方法之后）新增：

```rust
/// 插入供应商产品价格记录
pub async fn insert(
    executor: &mut sqlx::postgres::PgConnection,
    supplier_id: i64,
    product_id: i64,
    unit_price: Decimal,
    currency_code: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO supplier_product_prices
            (supplier_id, product_id, price, currency_code, min_order_qty)
        VALUES ($1, $2, $3, $4, 1)
        "#,
    )
    .bind(supplier_id)
    .bind(product_id)
    .bind(unit_price)
    .bind(currency_code)
    .execute(&mut *executor)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: 在 order/implt.rs 顶部增加导入**

在现有导入区域（`use crate::purchase::settings::repo::PurchaseSettingsRepo;` 之后）增加：

```rust
use crate::purchase::payment_schedule::{
    model::PaymentScheduleInput, new_payment_schedule_service, service::PaymentScheduleService,
};
use crate::purchase::supplier_price::repo::SupplierProductPriceRepo;
```

- [ ] **Step 3: 在 confirm() 方法末尾（审计日志步骤之后、`Ok(())` 之前）增加付款计划生成 + 供应商价格创建**

在 `confirm()` 方法的 `// 7. 审计日志` 块之后、`Ok(())` 之前插入：

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

        // 9. 自动创建缺失的供应商价格记录
        for item in &items {
            let existing = SupplierProductPriceRepo::match_best_price(
                &mut *db,
                order.supplier_id,
                item.product_id,
                item.quantity,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if existing.is_none() {
                SupplierProductPriceRepo::insert(
                    &mut *db,
                    order.supplier_id,
                    item.product_id,
                    item.unit_price,
                    &order.currency_code,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 5: 提交**

```bash
git add abt-core/src/purchase/order/implt.rs abt-core/src/purchase/supplier_price/repo.rs
git commit -m "feat(purchase): confirm() 生成付款计划 + 自动创建供应商价格记录"
```

---

### Task 2: payment approve() 三向匹配校验

**Files:**
- Modify: `abt-core/src/purchase/payment/implt.rs`

- [ ] **Step 1: 在 approve() 方法中，状态转换之前（步骤 2 之前）增加三向匹配**

在 `approve()` 方法的 `// 1. 获取当前记录` 块之后、`// 2. 状态转换` 之前插入：

```rust
        // 1.5 三向匹配校验：对账数量 ≤ 收货数量 + 金额一致性
        if let Some(recon_id) = payment.reconciliation_id {
            let recon_items = PurchaseReconItemRepo::list_by_reconciliation_id(
                &mut *db, recon_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            for item in &recon_items {
                let po_items = PurchaseOrderItemRepo::list_by_order_id(
                    &mut *db, item.order_id,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

                let po_item = po_items.iter().find(|p| p.id == item.order_item_id)
                    .ok_or_else(|| DomainError::validation(format!(
                        "订单行 {} 不存在", item.order_item_id
                    )))?;

                if item.received_qty > po_item.received_qty {
                    return Err(DomainError::validation(format!(
                        "对账数量 {} 超过收货数量 {}",
                        item.received_qty, po_item.received_qty
                    )));
                }

                let net_qty = item.received_qty - item.returned_qty;
                let expected_amount = net_qty * item.unit_price;
                let tolerance = expected_amount * Decimal::new(5, 1000);
                if (item.amount - expected_amount).abs() > tolerance {
                    return Err(DomainError::validation(format!(
                        "对账金额 {} 与净数量×单价 {} 不匹配（容差 0.5%）",
                        item.amount, expected_amount
                    )));
                }
            }
        }
```

- [ ] **Step 2: 增加缺失的导入**

确认 `payment/implt.rs` 顶部已有 `use crate::purchase::order::repo::{PurchaseOrderItemRepo, ...};`。如果没有，增加 `PurchaseOrderItemRepo` 到导入列表。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 4: 提交**

```bash
git add abt-core/src/purchase/payment/implt.rs
git commit -m "feat(purchase): payment approve() 增加三向匹配校验"
```

---

## Phase 2：Web UI

### Task 3: PO 创建/编辑页 — 税率下拉 + 折扣输入 + 汇总区

**Files:**
- Modify: `abt-web/src/pages/purchase_order_create.rs`
- Modify: `abt-web/src/pages/purchase_order_edit.rs`
- Modify: `abt-web/src/routes/purchase_order.rs`

- [ ] **Step 1: 创建页 handler 加载税率数据**

在 `get_po_create` handler 中，在加载 `quotations` 之后增加税率加载：

```rust
    let tax_rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
```

修改 `po_create_page` 函数签名增加 `tax_rates: &[TaxRate]` 参数，传入 `&tax_rates`。

- [ ] **Step 2: 创建页模板增加税率下拉到明细行表头**

在 `po_create_page` 函数中，明细表格 `<thead>` 的 `<th>` 列表中，在"小计"列之前增加：

```rust
                                th style="width:80px;text-align:right" { "折扣%" }
                                th style="width:120px" { "税率" }
```

- [ ] **Step 3: 创建页模板底部增加汇总区**

在明细表 `</table>` 之后、`</div>`（data-card 闭合）之前增加：

```rust
                    div style="display:flex;justify-content:flex-end;padding:var(--space-4);border-top:1px solid var(--border)" {
                        div style="display:flex;gap:var(--space-6);font-size:var(--text-sm)" {
                            div { "不含税: " span id="sum-untaxed" style="font-weight:600" { "0.00" } }
                            div { "税额: " span id="sum-tax" style="font-weight:600" { "0.00" } }
                            div { "含税总计: " span id="sum-total" style="font-weight:600;color:var(--primary)" { "0.00" } }
                        }
                    }
```

- [ ] **Step 4: 编辑页同样改动**

对 `purchase_order_edit.rs` 做相同改动：handler 加载税率、表头增加列、底部增加汇总区。`get_po_edit` handler 同样增加 `tax_rate_service().list_active()` 调用。

- [ ] **Step 5: 新增税率列表 JSON 路由**

在 `routes/purchase_order.rs` 中增加：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/tax-rates")]
pub struct TaxRatesPath;

// 在 router() 函数中增加：
.route(TaxRatesPath::PATH, get(purchase_order_create::get_tax_rates))
```

在 `purchase_order_create.rs` 中增加 handler：

```rust
#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_tax_rates(ctx: RequestContext) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    let json: Vec<serde_json::Value> = rates.iter().map(|r| serde_json::json!({
        "id": r.id, "code": r.code, "name": r.name, "rate": r.rate.to_string()
    })).collect();
    Ok(axum::Json(serde_json::Value::Array(json)))
}
```

- [ ] **Step 6: 验证编译**

Run: `cargo clippy 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 7: 提交**

```bash
git add abt-web/src/pages/purchase_order_create.rs abt-web/src/pages/purchase_order_edit.rs abt-web/src/routes/purchase_order.rs
git commit -m "feat(purchase): PO 创建/编辑页增加税率下拉+折扣输入+汇总区"
```

---

### Task 4: 前端 JS — calcPurchaseLine + updatePurchaseSummary + collectPurchaseItems

**Files:**
- Modify: `static/app.js`

- [ ] **Step 1: 在 app.js 末尾增加 calcPurchaseLine 函数**

```javascript
// ===== Purchase Order Line Calculation =====

function calcPurchaseLine(row) {
    const qty = parseFloat(row.querySelector('[data-field="qty"]')?.value) || 0;
    const price = parseFloat(row.querySelector('[data-field="price"]')?.value) || 0;
    const discount = parseFloat(row.querySelector('[data-field="discount"]')?.value) || 0;
    const taxRateId = row.querySelector('[data-field="tax_rate_id"]')?.value;
    const taxRate = taxRateId ? parseFloat(document.querySelector(`#tax-rate-${taxRateId}`)?.dataset?.rate) || 0 : 0;

    const subtotal = qty * price * (1 - discount / 100);
    const tax = subtotal * taxRate / 100;
    const total = subtotal + tax;

    const subtotalEl = row.querySelector('[data-field="subtotal"]');
    if (subtotalEl) subtotalEl.textContent = formatMoney(subtotal);

    return { subtotal, tax, total };
}

function updatePurchaseSummary() {
    let untaxed = 0, tax = 0, total = 0;
    document.querySelectorAll('tr[data-item-row]').forEach(row => {
        const r = calcPurchaseLine(row);
        untaxed += r.subtotal;
        tax += r.tax;
        total += r.total;
    });
    const elU = document.getElementById('sum-untaxed');
    const elT = document.getElementById('sum-tax');
    const elTotal = document.getElementById('sum-total');
    if (elU) elU.textContent = formatMoney(untaxed);
    if (elT) elT.textContent = formatMoney(tax);
    if (elTotal) elTotal.textContent = formatMoney(total);
}

function formatMoney(v) {
    return v.toFixed(2).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

// 绑定到 PO 创建/编辑页面的行输入变化
document.addEventListener('input', function(e) {
    const row = e.target.closest('tr[data-item-row]');
    if (row && e.target.matches('[data-field="qty"], [data-field="price"], [data-field="discount"], [data-field="tax_rate_id"]')) {
        updatePurchaseSummary();
    }
});
```

- [ ] **Step 2: 更新 collectItems 以传递 discount_pct 和 tax_rate_id**

找到现有的 `collectItems` 函数（如果存在），在每行的收集逻辑中增加：

```javascript
discount_pct: row.querySelector('[data-field="discount"]')?.value || '0',
tax_rate_id: row.querySelector('[data-field="tax_rate_id"]')?.value || '',
```

如果没有现有 `collectItems`，则在 `app.js` 中增加一个 `collectPurchaseItems` 函数用于 PO 页面。

- [ ] **Step 3: 提交**

```bash
git add static/app.js
git commit -m "feat(purchase): app.js 增加 calcPurchaseLine + updatePurchaseSummary"
```

---

### Task 5: PO 详情页 — 付款计划展示 + 修改明细 + 发票状态 + 审批操作

**Files:**
- Modify: `abt-web/src/pages/purchase_order_detail.rs`
- Modify: `abt-web/src/routes/purchase_order.rs`

- [ ] **Step 1: handler 加载付款计划数据**

在 `get_po_detail` handler 中，加载付款计划：

```rust
    let schedules = state.payment_schedule_service()
        .list_by_order(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();
```

将 `&schedules` 传入页面模板函数。

- [ ] **Step 2: 增加发票状态 label 函数**

在 `purchase_order_detail.rs` 中增加：

```rust
fn invoice_status_label(s: InvoiceStatus) -> (&'static str, &'static str) {
    match s {
        InvoiceStatus::NoInvoice => ("未开票", "status-draft"),
        InvoiceStatus::ToInvoice => ("待开票", "status-pending"),
        InvoiceStatus::FullyInvoiced => ("已开票", "status-completed"),
    }
}
```

需要导入 `use abt_core::purchase::enums::InvoiceStatus;`

- [ ] **Step 3: 详情页展示付款计划 data-card**

在订单信息 data-card 之后增加：

```rust
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "付款计划" }
                @if schedules.is_empty() {
                    p style="color:var(--text-tertiary);padding:var(--space-3)" { "暂无付款计划" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "期次" }
                                th { "到期日" }
                                th style="text-align:right" { "百分比" }
                                th style="text-align:right" { "应付金额" }
                                th style="text-align:right" { "已付金额" }
                            }
                        }
                        tbody {
                            @for (i, sched) in schedules.iter().enumerate() {
                                tr {
                                    td { (i + 1) }
                                    td { (sched.due_date.format("%Y-%m-%d").to_string()) }
                                    td style="text-align:right" { (format!("{}%", sched.payment_pct)) }
                                    td style="text-align:right" { (sched.payment_amount) }
                                    td style="text-align:right" { (sched.paid_amount) }
                                }
                            }
                        }
                    }
                }
            }
```

- [ ] **Step 4: 增加审批操作按钮**

在详情页操作区域，根据 status 条件渲染：

```rust
            @match order.status {
                PurchaseOrderStatus::Draft => {
                    form hx-post=(POSubmitPath { id: path.id }.to_string()) style="display:inline" {
                        button type="submit" class="btn btn-primary" { "提交审批" }
                    }
                }
                PurchaseOrderStatus::PendingApproval => {
                    form hx-post=(POApprovePath { id: path.id }.to_string()) style="display:inline" {
                        button type="submit" class="btn btn-primary" { "审批通过" }
                    }
                    button class="btn btn-danger"
                        _="on click add .is-open to #reject-modal" { "退回" }
                }
                _ => {}
            }
```

- [ ] **Step 5: 增加发票状态 badge**

在订单信息区域的状态 badge 旁边增加发票状态：

```rust
                            span class=("status-pill ")(invoice_label.1) {
                                (invoice_label.0)
                            }
```

- [ ] **Step 6: 新增路由定义**

在 `routes/purchase_order.rs` 中增加：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/submit")]
pub struct POSubmitPath { pub id: i64 }

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/approve")]
pub struct POApprovePath { pub id: i64 }

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/reject")]
pub struct PORejectPath { pub id: i64 }

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/items/update")]
pub struct POItemsUpdatePath { pub id: i64 }
```

在 `router()` 函数中注册这些路由，指向 `purchase_order_detail` 中的 handler。

- [ ] **Step 7: 在 detail 页面文件中增加 handler 函数**

```rust
pub async fn submit_po(/* ... */) -> Result<...> {
    // 调用 svc.submit(ctx, conn, id, None)
    // 返回 HX-Redirect 到详情页
}

pub async fn approve_po_order(/* ... */) -> Result<...> {
    // 调用 svc.approve_po(ctx, conn, id, None)
}

pub async fn reject_po(/* ... */) -> Result<...> {
    // 调用 svc.reject(ctx, conn, id, reason, None)
}

pub async fn update_po_items(/* ... */) -> Result<...> {
    // 解析 PoItemChange 列表
    // 调用 svc.update_items_after_confirm(ctx, conn, id, changes, None)
}
```

- [ ] **Step 8: 验证编译**

Run: `cargo clippy 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 9: 提交**

```bash
git add abt-web/src/pages/purchase_order_detail.rs abt-web/src/routes/purchase_order.rs
git commit -m "feat(purchase): PO 详情页增加付款计划+审批操作+发票状态"
```

---

### Task 6: PO 列表页 — 合并 PO + 状态筛选

**Files:**
- Modify: `abt-web/src/pages/purchase_order_list.rs`
- Modify: `abt-web/src/routes/purchase_order.rs`

- [ ] **Step 1: 列表页增加 checkbox 列 + 合并按钮**

在表格 `<thead>` 增加 `<th style="width:36px"><input type="checkbox" id="select-all" /></th>`

在每行 `<tr>` 增加 `<td><input type="checkbox" class="po-checkbox" value=(order.id) /></td>`

在操作区增加合并按钮：

```rust
button class="btn btn-sm btn-primary"
    _="on click call mergeSelectedPOs()"
{ "合并选中" }
```

- [ ] **Step 2: 增加合并路由 + handler**

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/merge")]
pub struct POMergePath;

// handler
pub async fn merge_orders_handler(ctx: RequestContext) -> Result<...> {
    // 解析 order_ids 参数
    // 调用 svc.merge_orders(ctx, conn, order_ids, None)
    // HX-Redirect
}
```

- [ ] **Step 3: 验证编译 + 提交**

Run: `cargo clippy 2>&1 | findstr /C:"error"`

```bash
git add abt-web/src/pages/purchase_order_list.rs abt-web/src/routes/purchase_order.rs
git commit -m "feat(purchase): PO 列表页增加合并功能"
```

---

### Task 7: 新管理页面 — 采购设置 + 审批规则 + 供应商价格

**Files:**
- Create: `abt-web/src/pages/purchase_settings.rs`
- Create: `abt-web/src/routes/purchase_settings.rs`
- Create: `abt-web/src/pages/purchase_approval_rules.rs`
- Create: `abt-web/src/routes/purchase_approval_rules.rs`
- Create: `abt-web/src/pages/supplier_price_catalog.rs`
- Create: `abt-web/src/routes/supplier_price_catalog.rs`
- Modify: `abt-web/src/routes/mod.rs`
- Modify: `abt-core/src/purchase/approval/service.rs` + `implt.rs` + `repo.rs` (增加 CRUD 方法)
- Modify: `abt-core/src/purchase/supplier_price/service.rs` + `implt.rs` + `repo.rs` (增加 CRUD 方法)

- [ ] **Step 1: approval service 增加 create_rule / delete_rule**

在 `approval/repo.rs` 增加 `insert` 和 `delete_by_id` 方法。
在 `approval/service.rs` trait 增加 `create_rule` 和 `delete_rule` 方法签名。
在 `approval/implt.rs` 实现这两个方法。

- [ ] **Step 2: supplier_price service 增加 create_price / delete_price**

在 `supplier_price/repo.rs` 增加 `delete_by_id` 方法（`insert` 已在 Task 1 中添加）。
在 `supplier_price/service.rs` trait 增加 `create_price` 和 `delete_price` 方法签名。
在 `supplier_price/implt.rs` 实现这两个方法。

- [ ] **Step 3: 创建采购设置页面**

`purchase_settings.rs`：GET handler 加载 `purchase_settings_service().get()`，渲染 data-card 表单；POST handler 解析表单调用 `.update()`。

路由：`GET /admin/purchase/settings` + `POST /admin/purchase/settings`

- [ ] **Step 4: 创建审批规则管理页面**

`purchase_approval_rules.rs`：GET handler 加载规则列表渲染 data-table；POST create handler 创建规则；POST delete handler 删除规则。

路由：`GET /admin/purchase/approval-rules` + `POST .../create` + `POST .../{id}/delete`

- [ ] **Step 5: 创建供应商价格目录页面**

`supplier_price_catalog.rs`：GET handler 按供应商/产品筛选加载列表；POST create handler 创建价格记录；POST delete handler 删除。

路由：`GET /admin/purchase/supplier-prices` + `POST .../create` + `POST .../{id}/delete`

- [ ] **Step 6: 在 routes/mod.rs 注册新路由模块**

```rust
mod purchase_settings;
mod purchase_approval_rules;
mod supplier_price_catalog;
```

在 master router 的 `merge` 链中注册各 router()。

- [ ] **Step 7: 验证编译**

Run: `cargo clippy 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 8: 提交**

```bash
git add abt-web/src/pages/ abt-web/src/routes/ abt-core/src/purchase/approval/ abt-core/src/purchase/supplier_price/
git commit -m "feat(purchase): 新增采购设置+审批规则+供应商价格管理页面"
```

---

### Task 8: 最终验证 + 设计文档同步

- [ ] **Step 1: 全量 clippy 验证**

Run: `cargo clippy 2>&1 | findstr /C:"error"`
Expected: 无 error 输出

- [ ] **Step 2: 更新设计文档**

在 `docs/uml-design/02-purchase.html` 的增强实现记录中补充 Web UI 完成信息。

- [ ] **Step 3: 提交**

```bash
git add docs/uml-design/02-purchase.html
git commit -m "docs: 更新采购模块设计文档——Web UI 补齐完成"
```
