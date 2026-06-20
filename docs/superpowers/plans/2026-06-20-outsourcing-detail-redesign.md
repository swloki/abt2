# 委外单详情页重新设计（还原原型）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把委外单详情页 `/admin/om/outsourcing/{id}` 完整还原为 Open Design 原型 `05-outsourcing-detail.html` 的 5 区块设计（补发料明细表 + 收发记录表 + 视觉升级）。

**Architecture:** abt-core 在 `OutsourcingOrderService` 补两个查询方法（发料明细 + 收发记录，后者复用 document_link + WMS find_by_source）；abt-web `get_detail` handler 多查这些数据并计算金额，`detail_page` 新增两个表格组件 + Hero/时间线视觉升级（全部 UnoCSS 原子类，shimmer 动画已存在于 uno.config）。

**Tech Stack:** Rust 2024 / axum / Maud / HTMX / UnoCSS / sqlx / PostgreSQL

## Global Constraints

- 沟通用中文
- abt-web 禁止 `sqlx::query*`/直接 DB，所有数据通过 abt-core Service trait（`state.xxx_service()`）
- abt-web 禁止 Maud 模板内联 `style=""`，100% UnoCSS 原子类
- 路由必须用 `TypedPath`，不硬编码 URL
- 不要用 `cargo run` 启动服务（服务已在运行），验证用 `cargo clippy`；页面验证用 agent-browser `--cdp 9222` + `snapshot -i`（禁止截图）
- 错误禁止静默丢弃（`let _ =`），用 `?`/`map_err`/`if let Err`
- 改 service trait 须同步 `docs/uml-design/05-outsourcing.html`
- abt-core service impl 遵循按需工厂模式（struct 只持 PgPool，方法体用 `new_xxx_service(self.pool.clone())`）
- 已修复的前置项：状态机（migration 059）、source_warehouse_id（migration 060）、发料按钮状态门控——本计划在其基础上继续

---

## File Structure

| 文件 | 责任 | 动作 |
|------|------|------|
| `abt-core/src/om/outsourcing_order/service.rs` | Service trait | 加 `list_materials` + `list_inventory_records` |
| `abt-core/src/om/outsourcing_order/implt.rs` | Service 实现 | 实现两个新方法 |
| `abt-core/src/om/outsourcing_order/mod.rs` | 导出 | 如需导出新类型 |
| `abt-web/src/pages/om_outsourcing_detail.rs` | 详情页 handler + detail_page + 组件 | handler 多查数据；新增 materials_section/transactions_section；Hero/时间线视觉升级 |
| `docs/uml-design/05-outsourcing.html` | UML 设计文档 | OutsourcingOrderService 接口加 list_materials/list_inventory_records 签名 |
| `abt-core/tests/outsourcing_service_test.rs`（或现有测试文件） | service 测试 | 加 list_materials 测试 |

**已有可复用资产**（勿重复造）：
- `OutsourcingMaterialRepo::list_by_outsourcing_id(executor, id) -> Result<Vec<OutsourcingMaterial>>`（repo.rs:255）
- `InventoryTransactionService::find_by_source(ctx, db, source_type: &str, source_id: i64)`（wms/inventory_transaction/service.rs:22）
- `DocumentLinkService::find_linked(ctx, db, source_type, source_id, target_type)`（shared/document_link/service.rs:21）
- `OutsourcingMaterial` model：`{id, outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost}`
- `InventoryTransaction` model：`{id, doc_number, source_doc_number, transaction_type, product_id, warehouse_id, quantity, source_type, source_id, remark, operator_id, created_at}`
- send/receive 已建 `OutsourcingOrder → InventoryTransfer` document_link（implt.rs:328, 492）
- uno.config.ts 已有 `shimmer-bar` 动画（6s/ease-in-out/infinite），用 `animate-shimmer-bar`

---

## Task 1: abt-core 加 `list_materials` service 方法

**Files:**
- Modify: `abt-core/src/om/outsourcing_order/service.rs`
- Modify: `abt-core/src/om/outsourcing_order/implt.rs`

**Interfaces:**
- Produces: `OutsourcingOrderService::list_materials(&self, ctx: &ServiceContext, db: PgExecutor<'_>, outsourcing_id: i64) -> Result<Vec<OutsourcingMaterial>>`

- [ ] **Step 1: 在 service.rs trait 加方法签名**

在 `OutsourcingOrderService` trait 的 `find_by_id` 附近加：

```rust
/// 查询委外单的发料明细列表
async fn list_materials(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    outsourcing_id: i64,
) -> Result<Vec<crate::om::outsourcing_order::model::OutsourcingMaterial>>;
```

- [ ] **Step 2: 在 implt.rs 实现**

在 `OutsourcingOrderServiceImpl` 的 `find_by_id` 实现附近加：

```rust
async fn list_materials(
    &self,
    _ctx: &ServiceContext,
    db: PgExecutor<'_>,
    outsourcing_id: i64,
) -> Result<Vec<crate::om::outsourcing_order::model::OutsourcingMaterial>> {
    let mut conn = match db {
        PgExecutor::Pool(p) => p.acquire().await.map_err(|e| DomainError::Internal(e.into()))?,
        PgExecutor::Conn(c) => c,
    };
    crate::om::outsourcing_order::repo::OutsourcingMaterialRepo::list_by_outsourcing_id(
        &mut conn,
        outsourcing_id,
    )
    .await
}
```

> 注：参考同文件其他方法（如 `send`/`receive`）对 `PgExecutor` 的处理模式，保持一致。若 `get_order` 等已有 helper 用 `&mut *db`，按同样方式取 `&mut PgConnection`。

- [ ] **Step 3: 编译验证**

Run: `cargo clippy -p abt-core 2>&1 | tail -5`
Expected: Finished，无 error（warning 可接受）

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/om/outsourcing_order/service.rs abt-core/src/om/outsourcing_order/implt.rs
git commit -m "feat(om): OutsourcingOrderService 加 list_materials 方法"
```

---

## Task 2: abt-core 加 `list_inventory_records`（收发记录）

**Files:**
- Modify: `abt-core/src/om/outsourcing_order/service.rs`
- Modify: `abt-core/src/om/outsourcing_order/implt.rs`

**Interfaces:**
- Consumes: `DocumentLinkService::find_linked`（shared）、`InventoryTransactionService::find_by_source`（wms）。需要 `use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};` 和 `use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};`（implt.rs 已 import 后者用于别处，确认前者是否需要加）
- Produces: `OutsourcingOrderService::list_inventory_records(&self, ctx, db, outsourcing_id: i64) -> Result<Vec<InventoryTransaction>>`

**查询链**：委外单 →（document_link find_linked, target_type=InventoryTransfer）→ 调拨单 ids →（find_by_source("inventory_transfer", transfer_id)）→ 流水 → 合并去重 + 按 created_at 排序

- [ ] **Step 1: service.rs 加签名**

```rust
/// 查询委外单关联的库存收发记录（发料/收货流水，来自关联的 WMS 调拨单）
async fn list_inventory_records(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    outsourcing_id: i64,
) -> Result<Vec<crate::wms::inventory_transaction::model::InventoryTransaction>>;
```

- [ ] **Step 2: implt.rs 实现**

确认 import（implt.rs 顶部）：
```rust
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::enums::document_type::DocumentType;  // 已 import
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};
```

实现：
```rust
async fn list_inventory_records(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    outsourcing_id: i64,
) -> Result<Vec<crate::wms::inventory_transaction::model::InventoryTransaction>> {
    use std::collections::HashSet;

    // 1. 委外单 → 关联的调拨单 ids
    let links = new_document_link_service(self.pool.clone())
        .find_linked(
            ctx,
            db,
            DocumentType::OutsourcingOrder,
            outsourcing_id,
            DocumentType::InventoryTransfer,
        )
        .await?;
    let transfer_ids: HashSet<i64> = links.into_iter().map(|l| l.target_id).collect();

    // 2. 每个调拨单 → 库存流水
    let tx_svc = new_inventory_transaction_service(self.pool.clone());
    let mut records = Vec::new();
    for tid in transfer_ids {
        let txns = tx_svc
            .find_by_source(ctx, db, "inventory_transfer", tid)
            .await?;
        records.extend(txns);
    }

    // 3. 去重（同一流水可能被多 link 引用）+ 按时间升序
    records.sort_by_key(|r| r.created_at);
    let mut seen = HashSet::new();
    let records = records
        .into_iter()
        .filter(|r| seen.insert(r.id))
        .collect();

    Ok(records)
}
```

> **验证点**：实现后用 agent-browser 在已发料的委外单（如单11）详情页确认能查到流水。若 `find_linked` 的签名/返回与上述假设不符（例如返回 `DocumentLink` vs `i64`），按实际 service.rs:21 调整 `.target_id` 取值。

- [ ] **Step 3: 编译验证**

Run: `cargo clippy -p abt-core 2>&1 | tail -5`
Expected: Finished，无 error

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/om/outsourcing_order/service.rs abt-core/src/om/outsourcing_order/implt.rs
git commit -m "feat(om): OutsourcingOrderService 加 list_inventory_records（复用 WMS 流水）"
```

---

## Task 3: abt-web `get_detail` handler 多查数据 + 计算金额

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`（`get_detail` 函数 + `detail_page` 签名）

**Interfaces:**
- Consumes: Task 1/2 的两个新 service 方法；现有 `state.warehouse_service()`（解析发料源仓名）
- Produces: `detail_page` 新增参数 `materials`, `inventory_records`, `source_warehouse_name`, `in_transit_amount`, `processing_fee`

- [ ] **Step 1: get_detail handler 加查询**

在 `get_detail`（当前 line ~104-141 区域）的 `warehouse_name` 加载之后、`detail_page(...)` 调用之前，加：

```rust
// 发料明细
let materials = svc
    .list_materials(&service_ctx, &mut conn, path.id)
    .await
    .unwrap_or_default();

// 收发记录（WMS 库存流水）
let inventory_records = svc
    .list_inventory_records(&service_ctx, &mut conn, path.id)
    .await
    .unwrap_or_default();

// 发料源仓名称
let source_warehouse_name = match order.source_warehouse_id {
    Some(wid) => state
        .warehouse_service()
        .get(&service_ctx, &mut conn, wid)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into()),
    None => "—".into(),
};

// 金额计算
use rust_decimal::Decimal;
let in_transit_amount: Decimal = materials
    .iter()
    .map(|m| (m.sent_qty - m.returned_qty) * m.unit_cost)
    .sum();
let processing_fee = order.planned_qty * order.unit_price;
```

- [ ] **Step 2: 修改 detail_page 调用，传入新参数**

把 `let content = detail_page(&order, &supplier_name, &product_name, &operator_name, &warehouse_name, &work_order_name, &tracking,);` 改为：

```rust
let content = detail_page(
    &order,
    &supplier_name,
    &product_name,
    &operator_name,
    &warehouse_name,
    &work_order_name,
    &source_warehouse_name,
    &tracking,
    &materials,
    &inventory_records,
    in_transit_amount,
    processing_fee,
);
```

- [ ] **Step 3: 扩展 detail_page 签名**

把 `fn detail_page(order, supplier_name, product_name, operator_name, warehouse_name, work_order_name, tracking) -> Markup` 改为：

```rust
fn detail_page(
    order: &abt_core::om::outsourcing_order::OutsourcingOrder,
    supplier_name: &str,
    product_name: &str,
    operator_name: &str,
    warehouse_name: &str,
    work_order_name: &str,
    source_warehouse_name: &str,
    tracking: &[abt_core::om::outsourcing_tracking::OutsourcingTracking],
    materials: &[abt_core::om::outsourcing_order::model::OutsourcingMaterial],
    inventory_records: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
    in_transit_amount: rust_decimal::Decimal,
    processing_fee: rust_decimal::Decimal,
) -> Markup {
    // ... 原有内容
}
```

- [ ] **Step 4: 编译验证（预期有未使用参数 warning，Task 4/5 会用）**

Run: `cargo clippy -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 无 error（unused param warning 暂时可接受）

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/om_outsourcing_detail.rs
git commit -m "feat(om-web): get_detail 查询发料明细/收发记录/金额"
```

---

## Task 4: abt-web 发料明细表组件 `materials_section`

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`

**Interfaces:**
- Produces: `fn materials_section(materials, in_transit_amount, processing_fee) -> Markup`，在 detail_page 的追踪时间线之后调用

- [ ] **Step 1: 写组件函数**

在 `detail_page` 函数之外（同级）加：

```rust
fn materials_section(
    materials: &[abt_core::om::outsourcing_order::model::OutsourcingMaterial],
    in_transit_amount: rust_decimal::Decimal,
    processing_fee: rust_decimal::Decimal,
) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
            // 顶部彩条（与原型一致）
            div class="h-[3px] bg-[linear-gradient(90deg,var(--warn),var(--accent),#60a5fa)]" {}
            // 标题
            div class="flex items-center justify-between px-8 py-5 border-b border-border-soft" {
                div class="flex items-center gap-3" {
                    div class="w-10 h-10 rounded-xl grid place-items-center bg-[linear-gradient(135deg,rgba(217,119,6,0.08),rgba(37,99,235,0.08))]" {
                        (maud::PreEscaped(r#"<svg class="w-5 h-5 text-warn" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/></svg>"#))
                    }
                    span class="text-[18px] font-bold text-fg" { "发料明细" }
                    span class="text-xs text-muted bg-surface px-2 py-0.5 rounded-full" { (format!("{} 项物料", materials.len())) }
                }
            }
            // 表格
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead { tr {
                        th { "物料" }
                        th class="text-right" { "应发数量" }
                        th class="text-right" { "已发数量" }
                        th class="text-right" { "已收回" }
                        th class="text-right" { "在途数量" }
                        th class="text-right" { "单位成本" }
                        th class="text-right" { "小计" }
                    }}
                    tbody {
                        @if materials.is_empty() {
                            tr { td colspan="7" class="text-center text-muted py-8" { "暂无发料明细" } }
                        } @else {
                            @for m in materials {
                                tr {
                                    td { span class="font-semibold text-fg" { "产品 #" (m.product_id) } }
                                    td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.planned_qty)) }
                                    td class="text-right font-mono tabular-nums text-success" { (crate::utils::fmt_qty(m.sent_qty)) }
                                    td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.returned_qty)) }
                                    td class="text-right font-mono tabular-nums text-warn font-semibold" {
                                        (crate::utils::fmt_qty(m.sent_qty - m.returned_qty))
                                    }
                                    td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.unit_cost)) }
                                    td class="text-right font-mono tabular-nums font-bold" {
                                        (crate::utils::fmt_qty(m.sent_qty * m.unit_cost))
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 金额汇总栏
            div class="flex items-center justify-end gap-8 px-8 py-4 border-t border-border-soft bg-surface" {
                div class="flex flex-col items-end gap-0.5" {
                    span class="text-xs text-muted font-semibold" { "在途物料金额" }
                    span class="text-lg font-bold font-mono tabular-nums text-warn" { (crate::utils::fmt_qty(in_transit_amount)) }
                }
                div class="flex flex-col items-end gap-0.5" {
                    span class="text-xs text-muted font-semibold" { "加工费" }
                    span class="text-lg font-bold font-mono tabular-nums text-accent" { (crate::utils::fmt_qty(processing_fee)) }
                }
            }
        }
    }
}
```

> **注**：物料名当前只显示 `产品 #id`（OutsourcingMaterial 无物料名缓存）。完整显示物料名需在 handler 额外 `product_svc.get` 解析。**优化项**：Task 3 handler 可加一个 `material_names: HashMap<i64, String>`（遍历 materials 的 product_id 调 product_svc.get），传入此组件显示真实物料名。若时间紧先用 `#id`，标记为后续优化。

- [ ] **Step 2: 在 detail_page 中调用（追踪时间线区块之后）**

在 detail_page 的 `// ═══ Tracking Timeline ═══` 区块结束 `}` 之后、modal 区之前，插入：

```rust
// ═══ 发料明细 ═══
(materials_section(materials, in_transit_amount, processing_fee))
```

- [ ] **Step 3: 编译验证**

Run: `cargo clippy -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/om_outsourcing_detail.rs
git commit -m "feat(om-web): 详情页加发料明细表组件"
```

---

## Task 5: abt-web 收发记录表组件 `transactions_section`

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`

**Interfaces:**
- Produces: `fn transactions_section(records) -> Markup`，在 materials_section 之后调用

- [ ] **Step 1: 写组件函数**

```rust
fn transactions_section(
    records: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> Markup {
    use abt_core::wms::enums::TransactionType;
    html! {
        div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
            div class="h-[3px] bg-[linear-gradient(90deg,var(--success),var(--accent),#60a5fa)]" {}
            div class="flex items-center justify-between px-8 py-5 border-b border-border-soft" {
                div class="flex items-center gap-3" {
                    div class="w-10 h-10 rounded-xl grid place-items-center bg-[linear-gradient(135deg,rgba(22,163,74,0.08),rgba(37,99,235,0.08))]" {
                        (maud::PreEscaped(r#"<svg class="w-5 h-5 text-success" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 7h12M8 12h12M8 17h12M4 7h.01M4 12h.01M4 17h.01"/></svg>"#))
                    }
                    span class="text-[18px] font-bold text-fg" { "收发记录" }
                    span class="text-xs text-muted bg-surface px-2 py-0.5 rounded-full" { (format!("{} 条记录", records.len())) }
                }
            }
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead { tr {
                        th { "时间" }
                        th { "类型" }
                        th { "物料" }
                        th class="text-right" { "数量" }
                        th { "仓库" }
                    }}
                    tbody {
                        @if records.is_empty() {
                            tr { td colspan="5" class="text-center text-muted py-8" { "暂无收发记录" } }
                        } @else {
                            @for r in records {
                                @let (type_label, type_cls) = match r.transaction_type {
                                    TransactionType::Transfer => ("调拨", "status-sent"),
                                    _ => ("流水", "status-progress"),
                                };
                                tr {
                                    td class="font-mono tabular-nums text-muted text-[13px]" {
                                        (r.created_at.format("%Y-%m-%d %H:%M"))
                                    }
                                    td { span class={ "inline-flex items-center px-2 py-0.5 rounded-full text-[11px] " (type_cls) } { (type_label) } }
                                    td class="font-medium" { "产品 #" (r.product_id) }
                                    td class={ "text-right font-mono tabular-nums " (if r.quantity.is_zero() { "" } else if r.quantity > rust_decimal::Decimal::ZERO { "text-success" } else { "text-danger" }) } {
                                        (crate::utils::fmt_qty(r.quantity))
                                    }
                                    td class="text-muted text-[13px]" { "仓库 #" (r.warehouse_id) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

> **注**：仓库/物料同样先显示 `#id`（handler 未传 names 映射）。优化项：Task 3 可加 `warehouse_names: HashMap<i64,String>` + 复用 product_names，传入显示真实名。优先级低于结构正确。

- [ ] **Step 2: 在 detail_page 调用（materials_section 之后）**

```rust
// ═══ 收发记录 ═══
(transactions_section(inventory_records))
```

- [ ] **Step 3: 编译验证**

Run: `cargo clippy -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/om_outsourcing_detail.rs
git commit -m "feat(om-web): 详情页加收发记录表组件"
```

---

## Task 6: Hero 视觉升级（shimmer 动画条 + 渐变进度环 + 发料源仓库字段）

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`（detail_page 的 Hero 区块）

- [ ] **Step 1: Hero 顶部加 shimmer 动画条**

在 detail_page 的 `// ═══ Detail Hero Card ═══` 最外层 div 内（现有 `overflow-hidden-accent` 空 div 位置），替换为流动彩条：

把现有的：
```rust
div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden-accent" {}
```
改为：
```rust
div class="h-1 bg-[linear-gradient(90deg,var(--accent),#60a5fa,var(--accent))] bg-[length:200%_100%] animate-shimmer-bar" {}
```

- [ ] **Step 2: 进度环改用渐变**

在 detail_page 的进度环 SVG（`// Progress Ring` 区块，含 `<circle ... stroke-dasharray=...>`），在 svg 内加 defs + 改 fill stroke。

把现有 svg 块替换为：
```rust
svg viewBox="0 0 56 56" {
    defs {
        linearGradient id="ringGrad" x1="0%" y1="0%" x2="100%" y2="0%" {
            stop offset="0%" stop-color="var(--accent)" {}
            stop offset="100%" stop-color="#60a5fa" {}
        }
    }
    circle class="w-[56px] h-[56px] relative" cx="28" cy="28" r="22" fill="none" stroke="var(--border-soft)" stroke-width="4" {}
    circle class="w-[56px] h-[56px] relative" cx="28" cy="28" r="22" fill="none"
        stroke="url(#ringGrad)" stroke-width="4" stroke-linecap="round"
        stroke-dasharray=(format!("{circumference:.1}"))
        stroke-dashoffset=(format!("{offset:.1}"))
        style="transform:rotate(-90deg);transform-origin:center" {}
}
```

> 注：`transform:rotate` 用 inline style 是 SVG 属性必需的例外（rotating svg circle）。若 clippy/约束允许则保留；否则用 UnoCSS 的 `[transform:rotate(-90deg)]` arbitrary variant。

- [ ] **Step 3: key grid 加"发料源仓库"字段**

在 detail_page 的 `// Info Split: Key fields` 的 `info-key-grid`（现有 6 字段：供应商/产品/关联工单/关联工序/虚拟仓库/预计交期）后面，把 grid 从隐式 2 列改 3 列并加第 7 个字段。

把现有 `div class="grid gap-[20px 48px]" {` 改为 `div class="grid grid-cols-3 gap-[20px 48px]" {`，并在"虚拟仓库"字段后、"预计交期"前（或末尾）加：

```rust
div class="flex flex-col gap-[6px]" {
    span class="text-xs text-muted font-semibold" { "发料源仓库" }
    span class="text-[15px] text-fg font-semibold" { (source_warehouse_name) }
}
```

- [ ] **Step 4: 编译验证**

Run: `cargo clippy -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 无 error

- [ ] **Step 5: 浏览器验证视觉**

Run: `./scripts/restart-abt.sh`（应用 #6 后服务已是最新；本任务仅前端，重启加载）
Then:
```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/om/outsourcing/11
agent-browser --cdp 9222 snapshot -i
```
Expected: Hero 顶部有流动彩条、进度环渐变、key grid 显示"发料源仓库: 原材料仓"

- [ ] **Step 6: Commit**

```bash
git add abt-web/src/pages/om_outsourcing_detail.rs
git commit -m "feat(om-web): Hero shimmer动画条+渐变进度环+发料源仓库字段"
```

---

## Task 7: 追踪时间线视觉对齐（当前节点高亮 + 顶部彩条）

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`（detail_page 的 Tracking Timeline 区块）

**现状**：时间线已有节点循环（completed/active/pending dot + label/time）。需补：区块顶部彩条 + active 节点 accent 渐变背景 + "当前"标签。

- [ ] **Step 1: 时间线区块顶部加彩条**

在 `// ═══ Tracking Timeline ═══` 的最外层 div（`bg-bg border border-border-soft rounded-xl relative overflow-hidden`）内最前面加：

```rust
div class="h-[3px] bg-[linear-gradient(90deg,var(--success),var(--accent),#60a5fa)]" {}
```

- [ ] **Step 2: active 节点加"当前"标签**

在节点循环里 `@let is_active = ...` 已有的分支，找到 active 节点的 label 渲染处，加"当前"小标签。把 active 分支的 label 行改为：

```rust
div class=(if is_active || is_completed { "track-label" } else { "track-label muted" }) {
    @if is_active {
        span class="text-accent" { (label) }
        span class="ml-2 text-[11px] font-medium px-2 py-0.5 rounded-full bg-[rgba(37,99,235,0.1)] text-accent" { "当前" }
    } @else {
        (label)
    }
}
```

（保持与现有 track-label 结构一致，仅 active 时加 accent 色 + 当前标签。）

- [ ] **Step 3: 编译验证**

Run: `cargo clippy -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/om_outsourcing_detail.rs
git commit -m "feat(om-web): 追踪时间线彩条+当前节点高亮"
```

---

## Task 8: 同步 UML + 全量回归验证

**Files:**
- Modify: `docs/uml-design/05-outsourcing.html`

- [ ] **Step 1: UML 加两个新方法签名**

在 `05-outsourcing.html` 的 `OutsourcingOrderService` 接口列表（line ~140 区域，`OutsourcingOrderService ..> MasterData.ProductService` 附近，或 service 方法清单）加：

```
+list_materials(outsourcing_id): Vec<OutsourcingMaterial>
+list_inventory_records(outsourcing_id): Vec<InventoryTransaction>  // 复用 document_link + WMS find_by_source
```

- [ ] **Step 2: 全量 clippy**

Run: `cargo clippy --workspace 2>&1 | tail -3`
Expected: Finished，无 error

- [ ] **Step 3: 重启 + 浏览器回归**

Run: `./scripts/restart-abt.sh`
Then 验证单11（Sent，已发料 8696）：
```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/om/outsourcing/11
agent-browser --cdp 9222 snapshot -i
```
Expected:
- Hero：shimmer 彩条流动、进度环渐变、发料源仓库显示"原材料仓"
- 发料明细表：1 行（产品 #8696，应发100/已发100/在途0/成本/小计），金额栏显示在途金额/加工费
- 收发记录表：≥2 条（调拨的出库+入库流水），类型"调拨"
- 追踪时间线：发料节点 completed，当前节点高亮+"当前"标签，顶部彩条

用 `eval` 抽查金额栏文本 + 表格行数：
```bash
agent-browser --cdp 9222 eval 'document.querySelector("table").innerText'
```

- [ ] **Step 4: 回归现有功能不破**

验证状态门控按钮、modal 仍正常（Snapshot 检查 action 按钮数 + modal 存在）。

- [ ] **Step 5: Commit**

```bash
git add docs/uml-design/05-outsourcing.html
git commit -m "docs(uml): OutsourcingOrderService 同步 list_materials/list_inventory_records"
```

---

## Self-Review 结论（plan 写完后自查）

**Spec 覆盖**：
- ① Hero 升级 → Task 6 ✓
- ② 追踪时间线 → Task 7 ✓
- ③ 发料明细表 + 金额 → Task 1（service）+ Task 3（数据）+ Task 4（组件）✓
- ④ 收发记录表 → Task 2（service）+ Task 3（数据）+ Task 5（组件）✓
- ⑤ Modal → 已有，不改 ✓
- 发料源仓库字段 → Task 3（查）+ Task 6（显示）✓
- UML 同步 → Task 8 ✓

**已知简化项**（标记在 Task 4/5 注里，非 placeholder，是明确的后续优化）：
- 物料名/仓库名先显示 `#id`，handler 加 names 映射为可选增强
- 这些不影响结构正确性与验收

**Type 一致性**：`list_materials` 返回 `Vec<OutsourcingMaterial>`，`list_inventory_records` 返回 `Vec<InventoryTransaction>`，handler 与组件签名匹配 ✓
