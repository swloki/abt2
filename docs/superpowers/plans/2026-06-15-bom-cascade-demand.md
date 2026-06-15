# BOM 级联需求展开 Implementation Plan v2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 销售订单确认后，自制产品的 BOM 原材料自动级联展开，缺口原材料自动生成采购需求进入采购需求池。

**Architecture:** 深度参考三家 ERP 的核心模式（详见 `docs/erp-comparison-sales-to-mrp-purchase.md` §5.2 设计要素参考矩阵），将每个设计决策追溯到具体 ERP 源码：

| 设计要素 | 参考 ERP | 源码位置 | ABT 落地点 |
|---|---|---|---|
| **projected_qty 库存可用量** | ERPNext | `bin.projected_qty = actual + ordered + planned − reserved` | 新增 `query_projected_qty` |
| **BOM 递归级联** | Odoo | `mrp/models/stock_rule.py:81` `_run_manufacture` 递归 | 新增 `explode_for_procurement` |
| **MTS 先查库存再下单** | Odoo | `stock_rule.py:304` `mts_else_mto` | projected_qty 扣减后再创建 Demand |
| **需求去重** | Odoo | `stock_rule.py:91` `_make_mo_get_domain` 查已有 MO 追加 | 新增 `find_cascade_existing` |
| **需求按物料聚合** | Odoo | `purchase_stock/stock_rule.py:87` `_make_po_get_domain` 按 supplier 合并 PO 行 | 已有 `find_material_aggregated` |
| **UI 行状态着色** | Odoo | `mrp_production_views.xml:69` `decoration-success/warning/danger` | SO 详情页履行行着色 |
| **UI Smart Button** | Odoo | `sale_order_views.xml:411` `oe_button_box` | SO 详情页关联单据统计 |
| **需求池事件驱动** | ABT 自有 | `DemandService.create_from_order` + Event Handler | 保持不动 |

**Tech Stack:** Rust (edition 2024), sqlx (PostgreSQL compile-time checked), Maud + HTMX + Hyperscript, UnoCSS

---

## File Structure

| 文件 | 职责 | 操作 | 参考 ERP |
|---|---|---|---|
| `abt-core/src/wms/stock_ledger/service.rs` | 新增 `query_projected_qty` trait 方法 | Modify | ERPNext |
| `abt-core/src/wms/stock_ledger/implt.rs` | 实现 projected_qty 计算 | Modify | ERPNext |
| `abt-core/src/wms/stock_ledger/repo.rs` | 新增 projected_qty SQL（含 PO 在途 + WO 在制） | Modify | ERPNext |
| `abt-core/src/master_data/bom/service.rs` | 新增 `explode_for_procurement` + `ProcurementRequirement` | Modify | Odoo |
| `abt-core/src/master_data/bom/implt.rs` | 实现 BOM 递归展开 | Modify | Odoo |
| `abt-core/src/master_data/bom/mod.rs` | 导出新类型 | Modify | — |
| `abt-core/src/sales/sales_order/model.rs` | `DemandInput` 新增 `cascade_from_product_id` | Modify | Odoo |
| `abt-core/src/sales/sales_order/repo.rs` | `DemandRepo` 新增 `find_cascade_existing` + `create` 更新 | Modify | Odoo |
| `abt-core/src/sales/sales_order/implt.rs` | `create_from_order` 新增 BOM 级联 + projected_qty 扣减 | Modify | Odoo + ERPNext |
| `abt-core/migrations/043_demands_cascade_from.sql` | demands 表加列 + 索引 | Create | — |
| `abt-web/src/pages/sales_order_detail.rs` | Smart Button + 行着色 | Modify | Odoo |
| `docs/uml-design/04-mes.html` | 设计文档同步 | Modify | — |

---

## Task 1: projected_qty 库存可用量（参考 ERPNext）

> **ERP 参考**：ERPNext `bin.projected_qty = actual_qty + ordered_qty + planned_qty − reserved_qty − reserved_for_production`
>
> **ABT 现状**：`StockLedgerRepo::total_available` 只算 `actual − reserved`，不考虑在途采购量(ordered)和在制工单量(planned)。导致原材料已有在途 PO 时仍重复创建采购需求。
>
> **改进**：新增 `query_projected_qty` 方法，一次 SQL 查出四维可用量。

**Files:**
- Modify: `abt-core/src/wms/stock_ledger/service.rs:12-33`
- Modify: `abt-core/src/wms/stock_ledger/repo.rs:85-112`
- Modify: `abt-core/src/wms/stock_ledger/implt.rs:57-67`

- [ ] **Step 1: 在 StockLedgerService trait 中新增方法**

在 `abt-core/src/wms/stock_ledger/service.rs` 的 `StockLedgerService` trait 中，`query_available` 之后添加：

```rust
    /// 预计可用量（参考 ERPNext projected_qty 公式）
    ///
    /// projected = actual + on_order_po + in_progress_wo - reserved
    ///
    /// - actual: 当前实物库存（stock_ledger.quantity）
    /// - on_order_po: 在途采购量（采购订单行 quantity - received_qty, PO 状态 Confirmed/PartiallyReceived）
    /// - in_progress_wo: 在制工单量（工单 planned_qty - completed_qty, 状态 Released/InProduction）
    /// - reserved: 硬预留量（inventory_reservations Active）
    async fn query_projected_qty(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<ProjectedQty>;
```

在 `service.rs` 顶部（use 区域之后）新增返回类型：

```rust
/// 预计可用量分解（参考 ERPNext projected_qty 四维公式）
#[derive(Debug, Clone)]
pub struct ProjectedQty {
    /// 当前实物库存
    pub actual: rust_decimal::Decimal,
    /// 在途采购量（未到货的 PO 数量）
    pub on_order_po: rust_decimal::Decimal,
    /// 在制工单量（未完工的 WO 数量）
    pub in_progress_wo: rust_decimal::Decimal,
    /// 硬预留量
    pub reserved: rust_decimal::Decimal,
    /// 净预计可用量 = actual + on_order_po + in_progress_wo - reserved
    pub projected: rust_decimal::Decimal,
}
```

- [ ] **Step 2: 在 StockLedgerRepo 中实现 projected_qty SQL**

在 `abt-core/src/wms/stock_ledger/repo.rs` 的 `StockLedgerRepo` impl 中，`total_available` 之后（约 line 113）添加：

```rust
    /// 预计可用量（参考 ERPNext bin.projected_qty 公式）
    ///
    /// 四维计算：
    /// 1. actual = SUM(stock_ledger.quantity)
    /// 2. on_order_po = SUM(purchase_order_items.quantity - received_qty)
    ///    WHERE po.status IN (2=Confirmed, 3=PartiallyReceived) AND poi.deleted_at IS NULL
    /// 3. in_progress_wo = SUM(work_orders.planned_qty - completed_qty)
    ///    WHERE wo.status IN (3=Released, 6=InProduction)
    /// 4. reserved = SUM(inventory_reservations.reserved_qty WHERE status=Active)
    ///
    /// projected = actual + on_order_po + in_progress_wo - reserved
    pub async fn projected_qty(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<crate::wms::stock_ledger::service::ProjectedQty> {
        use crate::wms::stock_ledger::service::ProjectedQty;
        use rust_decimal::Decimal;
        use sqlx::Row;

        let row = sqlx::query(
            r#"
            SELECT
                COALESCE((
                    SELECT SUM(sl.quantity) FROM stock_ledger sl
                    WHERE sl.product_id = $1 AND ($2::bigint IS NULL OR sl.warehouse_id = $2)
                ), 0) AS actual,
                COALESCE((
                    SELECT SUM(poi.quantity - poi.received_qty)
                    FROM purchase_order_items poi
                    JOIN purchase_orders po ON po.id = poi.order_id
                    WHERE poi.product_id = $1
                      AND po.status IN (2, 3)
                      AND po.deleted_at IS NULL
                ), 0) AS on_order_po,
                COALESCE((
                    SELECT SUM(wo.planned_qty - wo.completed_qty)
                    FROM work_orders wo
                    WHERE wo.product_id = $1
                      AND wo.status IN (3, 6)
                      AND wo.deleted_at IS NULL
                ), 0) AS in_progress_wo,
                COALESCE((
                    SELECT SUM(ir.reserved_qty)
                    FROM inventory_reservations ir
                    WHERE ir.product_id = $1
                      AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
                      AND ir.status = $3
                ), 0) AS reserved
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(crate::wms::inventory_reservation::model::ReservationStatus::Active)
        .fetch_one(executor)
        .await?;

        let actual: Decimal = row.try_get("actual")?;
        let on_order_po: Decimal = row.try_get("on_order_po")?;
        let in_progress_wo: Decimal = row.try_get("in_progress_wo")?;
        let reserved: Decimal = row.try_get("reserved")?;
        let projected = actual + on_order_po + in_progress_wo - reserved;

        Ok(ProjectedQty { actual, on_order_po, in_progress_wo, reserved, projected })
    }
```

**注意**：`ReservationStatus::Active` 的实际路径需确认。搜索 `inventory_reservation` 模块中 `ReservationStatus` 的定义。如果 `Active` 的 i16 值不是 1，调整 SQL。

- [ ] **Step 3: 在 StockLedgerServiceImpl 中实现 trait 方法**

在 `abt-core/src/wms/stock_ledger/implt.rs` 的 `StockLedgerService` impl 中（约 line 67 之后）添加：

```rust
    async fn query_projected_qty(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<crate::wms::stock_ledger::service::ProjectedQty> {
        StockLedgerRepo::projected_qty(&mut *db, product_id, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
```

- [ ] **Step 4: 更新 mod.rs 导出**

在 `abt-core/src/wms/stock_ledger/mod.rs` 的 `pub use` 行中添加：

```rust
pub use service::{StockLedgerService, ProjectedQty};
```

- [ ] **Step 5: 运行 clippy 验证编译通过**

Run: `cargo clippy -p abt-core 2>&1 | head -30`

Expected: 无编译错误。

- [ ] **Step 6: Commit**

```bash
git add abt-core/src/wms/stock_ledger/
git commit -m "feat(stock): add projected_qty (ERPNext formula: actual + on_order_po + in_progress_wo - reserved)"
```

---

## Task 2: BOM 递归展开（参考 Odoo `_run_manufacture`）

> **ERP 参考**：Odoo `mrp/models/stock_rule.py:81` `_run_manufacture`
> - 查 BOM (`_get_matching_bom`) → 创建 MO → MO 确认时展开原材料 stock.move → 每个原材料又触发 `StockRule.run()` 递归
> - 外购原材料走 `_run_buy`，自制子件递归走 `_run_manufacture`
>
> **ABT 落地**：不做 Odoo 那种 MO→move→procurement 的运行时递归链（ABT 是 Demand + Event 架构），而是在 `explode_for_procurement` 中一次性递归展开整棵 BOM 树，返回所有外购原材料的净需求。

**Files:**
- Modify: `abt-core/src/master_data/bom/service.rs`
- Modify: `abt-core/src/master_data/bom/implt.rs`
- Modify: `abt-core/src/master_data/bom/mod.rs`

- [ ] **Step 1: 在 service.rs 中新增 trait 方法和返回类型**

在 `abt-core/src/master_data/bom/service.rs` 中，`BomQueryService` trait 内、`find_published_bom_by_product_code` 之后添加：

```rust
    /// 递归展开 BOM，返回所有需采购的原材料及其净需求量
    ///
    /// 参考 Odoo `_run_manufacture` 递归模式：
    /// 1. 查 product_code 的已发布 BOM（`_get_matching_bom` 等价）
    /// 2. 遍历 BOM 节点，按 acquire_channel 分流：
    ///    - Purchased → 加入结果（乘 loss_rate）
    ///    - SelfProduced/Legacy → 递归展开该子件 BOM
    ///    - Outsourced → 加入结果（需采购原材料发给委外供应商）
    ///    - NonInventory → 跳过
    /// 3. 深度限制 10 层 + visited set 防循环引用
    async fn explode_for_procurement(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
        quantity: rust_decimal::Decimal,
    ) -> Result<Vec<ProcurementRequirement>>;
```

在 `service.rs` 顶部 `use` 之后添加：

```rust
/// BOM 展开采购需求项（对应 Odoo Procurement NamedTuple）
#[derive(Debug, Clone)]
pub struct ProcurementRequirement {
    /// 原材料 product_id
    pub product_id: i64,
    /// 净需求量（已含 loss_rate 损耗系数）
    pub required_qty: rust_decimal::Decimal,
    /// BOM 层级深度（0=成品直接子件，1=二级子件...）
    pub bom_level: u8,
}
```

- [ ] **Step 2: 在 mod.rs 中导出新类型**

在 `abt-core/src/master_data/bom/mod.rs` 的 `pub use service::` 行中添加 `ProcurementRequirement`：

```rust
pub use service::{BomCategoryService, BomCommandService, BomCostService, BomNodeService, BomQueryService, ProcurementRequirement};
```

- [ ] **Step 3: 给 BomQueryServiceImpl 添加 pool 字段**

在 `abt-core/src/master_data/bom/implt.rs` 中修改 struct 和 new（约 line 20-31）：

当前：
```rust
pub struct BomQueryServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
}

impl BomQueryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        let _ = pool;
        Self { repo: BomRepo, node_repo: BomNodeRepo, snapshot_repo: BomSnapshotRepo }
    }
}
```

改为：
```rust
pub struct BomQueryServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
    pool: PgPool,
}

impl BomQueryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomRepo, node_repo: BomNodeRepo, snapshot_repo: BomSnapshotRepo, pool }
    }
}
```

- [ ] **Step 4: 实现 explode_for_procurement + 递归辅助方法**

在 `abt-core/src/master_data/bom/implt.rs` 的 `BomQueryService` impl 块中，`find_published_bom_by_product_code` 之后添加 trait 方法实现：

```rust
    async fn explode_for_procurement(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
        quantity: rust_decimal::Decimal,
    ) -> Result<Vec<ProcurementRequirement>> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.explode_recursive(ctx, db, product_code, quantity, 0, &mut visited, &mut result).await?;
        Ok(result)
    }
```

在 `BomQueryServiceImpl` 的非 trait `impl` 块中（struct 方法），添加递归辅助：

```rust
    /// BOM 递归展开辅助方法
    ///
    /// 参考 Odoo _run_manufacture → MrpProduction.action_confirm → stock.move → StockRule.run 递归链。
    /// ABT 简化为一次性遍历，不做运行时 MO→move 链。
    async fn explode_recursive(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
        quantity: rust_decimal::Decimal,
        depth: u8,
        visited: &mut std::collections::HashSet<i64>,
        result: &mut Vec<ProcurementRequirement>,
    ) -> Result<()> {
        use crate::master_data::product::{new_product_service, service::ProductService};
        use crate::master_data::product::model::AcquireChannel;
        use rust_decimal::Decimal;

        if depth >= 10 {
            tracing::warn!(product_code, depth, "BOM explosion depth limit reached");
            return Ok(());
        }

        let bom_id = self.repo.find_published_by_product_code(db, product_code).await?;
        let Some(bom_id) = bom_id else { return Ok(()) };

        let nodes = self.node_repo.find_by_bom_id(db, bom_id).await?;

        let product_ids: Vec<i64> = nodes.iter().map(|n| n.product_id).collect();
        let products = new_product_service(self.pool.clone())
            .get_by_ids(ctx, db, product_ids).await?;
        let product_map: std::collections::HashMap<i64, (AcquireChannel, String)> = products
            .into_iter()
            .map(|p| (p.product_id, (p.acquire_channel, p.product_code)))
            .collect();

        for node in &nodes {
            if node.parent_id == 0 { continue }

            if visited.contains(&node.product_id) {
                tracing::warn!(product_id = node.product_id, "Circular BOM reference, skipping");
                continue;
            }

            // Odoo 模式：net_qty = bom_line_qty × parent_demand × (1 + loss_rate)
            let loss_multiplier = Decimal::ONE + node.loss_rate;
            let node_qty = node.quantity * quantity * loss_multiplier;

            let (ac, child_code) = product_map.get(&node.product_id)
                .map(|(ac, code)| (*ac, code.clone()))
                .unwrap_or((AcquireChannel::Legacy, String::new()));

            match ac {
                AcquireChannel::Purchased | AcquireChannel::Outsourced => {
                    // Odoo _run_buy：外购/委外件直接加入采购需求
                    result.push(ProcurementRequirement {
                        product_id: node.product_id,
                        required_qty: node_qty,
                        bom_level: depth,
                    });
                }
                AcquireChannel::SelfProduced | AcquireChannel::Legacy => {
                    // Odoo _run_manufacture：自制件递归展开子 BOM
                    visited.insert(node.product_id);
                    Box::pin(self.explode_recursive(
                        ctx, db, &child_code, node_qty, depth + 1, visited, result,
                    )).await?;
                    visited.remove(&node.product_id);
                }
                AcquireChannel::NonInventory => { /* 费用/服务类：跳过 */ }
            }
        }
        Ok(())
    }
```

- [ ] **Step 5: 运行 clippy 验证编译通过**

Run: `cargo clippy -p abt-core 2>&1 | head -30`

Expected: 无编译错误。

- [ ] **Step 6: Commit**

```bash
git add abt-core/src/master_data/bom/
git commit -m "feat(bom): implement explode_for_procurement (Odoo _run_manufacture recursive pattern)"
```

---

## Task 3: Demand 模型扩展（参考 Odoo procurement 来源追踪）

> **ERP 参考**：Odoo 的 `Procurement` NamedTuple 携带 `origin`（来源单据）和 `values`（上下文字典），可以追溯到是哪个 SO 的哪个成品展开的。ABT 用 `cascade_from_product_id` 实现等价追踪。
>
> **ABT 现状**：`DemandInput` 没有级联来源字段，无法区分「SO 直接需求」和「BOM 展开需求」。

**Files:**
- Modify: `abt-core/src/sales/sales_order/model.rs:487-521`
- Modify: `abt-core/src/sales/sales_order/repo.rs:568-602`
- Create: `abt-core/migrations/043_demands_cascade_from.sql`

- [ ] **Step 1: 数据库迁移 — 添加 cascade_from_product_id 列**

创建 `abt-core/migrations/043_demands_cascade_from.sql`：

```sql
-- 043: 为 demands 表添加 BOM 级联来源字段
-- 参考 Odoo Procurement.values['origin'] 的来源追踪机制
ALTER TABLE demands
    ADD COLUMN IF NOT EXISTS cascade_from_product_id BIGINT REFERENCES products(product_id);

-- 级联需求去重查询索引（Odoo _make_mo_get_domain 等价）
CREATE INDEX IF NOT EXISTS idx_demands_cascade
    ON demands (source_id, source_line_id, cascade_from_product_id, product_id)
    WHERE deleted_at IS NULL AND demand_type = 2;

COMMENT ON COLUMN demands.cascade_from_product_id IS
    'BOM展开来源产品ID。demand_type=2时记录此原材料属于哪个成品的BOM。NULL表示直接需求(demand_type=1)';
```

- [ ] **Step 2: 在 DemandInput 中添加字段**

在 `abt-core/src/sales/sales_order/model.rs` 的 `DemandInput`（约 line 509）中，`remark` 之前添加：

```rust
    /// BOM 展开来源产品 ID（Odoo Procurement.values['origin'] 等价）
    /// demand_type=2 时记录原材料属于哪个成品的 BOM
    pub cascade_from_product_id: Option<i64>,
```

- [ ] **Step 3: 在 Demand 实体中添加字段**

在同一文件 `Demand` struct（约 line 487-506）中，`remark` 之后添加：

```rust
    pub cascade_from_product_id: Option<i64>,
```

- [ ] **Step 4: 更新 DEMAND_COLUMNS 常量**

在 `abt-core/src/sales/sales_order/repo.rs`（约 line 573）：

```rust
const DEMAND_COLUMNS: &str = "id, demand_type, source_type, source_id, source_line_id, product_id, acquire_channel, required_qty, required_date, status, target_doc_type, target_doc_id, priority, cascade_from_product_id, remark, operator_id, created_at, updated_at, deleted_at";
```

- [ ] **Step 5: 更新 DemandRepo::create INSERT**

在 `abt-core/src/sales/sales_order/repo.rs` 的 `DemandRepo::create`（约 line 577-601）：

```rust
    pub async fn create(
        executor: PgExecutor<'_>,
        input: &DemandInput,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO demands
               (demand_type, source_type, source_id, source_line_id, product_id,
                acquire_channel, required_qty, required_date, status, priority,
                cascade_from_product_id, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1, $9, $10, $11, $12)
               RETURNING id"#,
        )
        .bind(input.demand_type)
        .bind(input.source_type)
        .bind(input.source_id)
        .bind(input.source_line_id)
        .bind(input.product_id)
        .bind(input.acquire_channel)
        .bind(input.required_qty)
        .bind(input.required_date)
        .bind(input.priority)
        .bind(input.cascade_from_product_id)
        .bind(&input.remark)
        .bind(input.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }
```

- [ ] **Step 6: 修复现有 DemandInput 构造点**

在 `abt-core/src/sales/sales_order/implt.rs` 的 `create_from_order`（约 line 958），现有 DemandInput 构造添加字段：

```rust
            let input = DemandInput {
                demand_type: 1,
                source_type: DocumentType::SalesOrder as i16,
                source_id: order_id,
                source_line_id: line.order_line_id,
                product_id: line.product_id,
                acquire_channel: line.acquire_channel.as_i16(),
                required_qty: line.shortage_qty,
                required_date: line.required_date,
                priority: 5,
                cascade_from_product_id: None,
                remark: String::new(),
                operator_id: ctx.operator_id,
            };
```

- [ ] **Step 7: 运行迁移**

Run: `psql $DATABASE_URL -f abt-core/migrations/043_demands_cascade_from.sql`

Expected: 迁移成功。

- [ ] **Step 8: 运行 clippy 验证**

Run: `cargo clippy -p abt-core 2>&1 | head -30`

Expected: 无编译错误。

- [ ] **Step 9: Commit**

```bash
git add abt-core/migrations/043_demands_cascade_from.sql abt-core/src/sales/sales_order/model.rs abt-core/src/sales/sales_order/repo.rs abt-core/src/sales/sales_order/implt.rs
git commit -m "feat(demand): add cascade_from_product_id for BOM-exploded demand origin tracking"
```

---

## Task 4: BOM 级联 + projected_qty 扣减（核心逻辑）

> **ERP 参考**：
> - **Odoo `mts_else_mto`**（`stock_rule.py:304`）：先查库存 MTS，不够再走 MTO 下单。ABT 用 projected_qty 判断是否需要创建采购需求。
> - **Odoo `_make_mo_get_domain`**（`mrp/stock_rule.py:146`）：查已有同源 MO，有则追加数量。ABT 用 `find_cascade_existing` 去重。
> - **ERPNext `projected_qty`**：扣减在途/在制后的净需求。

**Files:**
- Modify: `abt-core/src/sales/sales_order/implt.rs:937-999`
- Modify: `abt-core/src/sales/sales_order/repo.rs:656+`

- [ ] **Step 1: 新增 DemandRepo::find_cascade_existing（Odoo _make_mo_get_domain 去重）**

在 `abt-core/src/sales/sales_order/repo.rs` 的 `DemandRepo` impl 中（约 line 656 之后）添加：

```rust
    /// 检查是否已存在同来源的 BOM 级联需求
    ///
    /// 参考 Odoo `_make_mo_get_domain`：查已有 draft/confirmed 状态的同源 MO，
    /// 有则追加数量，无则新建。ABT 简化为存在性检查（不追加）。
    pub async fn find_cascade_existing(
        executor: PgExecutor<'_>,
        source_id: i64,
        source_line_id: i64,
        product_id: i64,
        cascade_from_product_id: i64,
    ) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM demands
               WHERE source_id = $1
                 AND source_line_id = $2
                 AND product_id = $3
                 AND cascade_from_product_id = $4
                 AND demand_type = 2
                 AND deleted_at IS NULL"#,
        )
        .bind(source_id)
        .bind(source_line_id)
        .bind(product_id)
        .bind(cascade_from_product_id)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }
```

- [ ] **Step 2: 在 create_from_order 中添加 BOM 级联逻辑**

在 `abt-core/src/sales/sales_order/implt.rs` 的 `DemandServiceImpl` impl 中，`create_from_order` 方法的现有 `for line in &fp_lines` 循环之后、`Ok(demand_ids)` 之前（约 line 997），添加级联逻辑。

首先确保文件顶部有以下 imports（约 line 1-15）。需添加：

```rust
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService, ProcurementRequirement};
use crate::master_data::product::new_product_service;
use crate::wms::stock_ledger::{new_stock_ledger_service, service::StockLedgerService};
```

然后在 `create_from_order` 中，`Ok(demand_ids)` 之前添加：

```rust
        // ════════════════════════════════════════════════════════════════════
        // BOM 级联展开：自制缺货行的原材料自动生成采购需求
        //
        // 参考：
        // - Odoo _run_manufacture: MO 创建后原材料 stock.move 触发 StockRule.run 递归
        // - Odoo mts_else_mto: 先查库存(projected_qty)，不够才创建需求
        // - Odoo _make_mo_get_domain: 查已有同源需求避免重复创建
        // - ERPNext projected_qty: actual + ordered + planned - reserved
        // ════════════════════════════════════════════════════════════════════

        let mut cascade_demands: Vec<DemandInput> = Vec::new();

        for line in &fp_lines {
            // 只对自制类缺货行做 BOM 展开
            if line.acquire_channel != AcquireChannel::SelfProduced
                && line.acquire_channel != AcquireChannel::Legacy
            {
                continue;
            }

            // 查成品的 product_code（BOM 查找入口）
            let product = new_product_service(self.pool.clone())
                .get(ctx, db, line.product_id).await?;
            let product_code = product.product_code.clone();

            // 递归展开 BOM（Odoo _run_manufacture 递归模式）
            let bom_reqs = new_bom_query_service(self.pool.clone())
                .explode_for_procurement(ctx, db, &product_code, line.shortage_qty)
                .await?;

            if bom_reqs.is_empty() {
                continue;
            }

            // 批量查询原材料 projected_qty（ERPNext 公式 + Odoo mts_else_mto）
            // 库存充足的原材料不生成采购需求
            for req in &bom_reqs {
                let projected = new_stock_ledger_service(self.pool.clone())
                    .query_projected_qty(ctx, db, req.product_id, None)
                    .await?;

                // Odoo mts_else_mto：projected 够则跳过
                let net_shortage = (req.required_qty - projected.projected).max(Decimal::ZERO);
                if net_shortage <= Decimal::ZERO {
                    continue;
                }

                // Odoo _make_mo_get_domain 去重：同 SO 行 + 同原材料 + 同来源成品
                let exists = DemandRepo::find_cascade_existing(
                    db, order_id, line.order_line_id, req.product_id, line.product_id,
                ).await?;
                if exists {
                    continue;
                }

                cascade_demands.push(DemandInput {
                    demand_type: 2,  // BOM 展开需求
                    source_type: DocumentType::SalesOrder as i16,
                    source_id: order_id,
                    source_line_id: line.order_line_id,
                    product_id: req.product_id,
                    acquire_channel: AcquireChannel::Purchased.as_i16(),
                    required_qty: net_shortage,
                    required_date: line.required_date,
                    priority: 5,
                    cascade_from_product_id: Some(line.product_id),
                    remark: format!(
                        "BOM展开: 成品{} 层{} 总需{} 预计可用{} 净缺{}",
                        line.product_id, req.bom_level, req.required_qty, projected.projected, net_shortage
                    ),
                    operator_id: ctx.operator_id,
                });
            }
        }

        // 创建级联需求 + 发布 DemandCreated 事件
        for input in &cascade_demands {
            let demand_id = DemandRepo::create(&mut *db, input).await?;
            demand_ids.push(demand_id);

            savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            if let Err(e) = new_domain_event_bus(self.pool.clone())
                .publish(ctx, db, EventPublishRequest {
                    event_type: DomainEventType::DemandCreated,
                    aggregate_type: "Demand".to_string(),
                    aggregate_id: demand_id,
                    payload: serde_json::json!({
                        "order_id": order_id,
                        "product_id": input.product_id,
                        "acquire_channel": input.acquire_channel,
                        "cascade_from": input.cascade_from_product_id,
                    }),
                    idempotency_key: None,
                })
                .await
            {
                tracing::warn!("Cascade DemandCreated event failed for demand {demand_id}: {e}");
                rollback_savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            } else {
                release_savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            }
        }

        if !cascade_demands.is_empty() {
            tracing::info!(
                "Created {} BOM cascade demands for order {order_id} (Odoo _run_manufacture pattern)",
                cascade_demands.len()
            );
        }
```

- [ ] **Step 3: 运行 clippy 验证编译通过**

Run: `cargo clippy -p abt-core 2>&1 | head -40`

Expected: 无编译错误。

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/sales/sales_order/implt.rs abt-core/src/sales/sales_order/repo.rs
git commit -m "feat(demand): cascade BOM raw material demands with projected_qty deduction (Odoo + ERPNext)"
```

---

## Task 5: 采购需求池视图更新

> **ERP 参考**：Odoo 在 PO 行上关联 `origin`（来源 SO），采购员可以看到这个采购是哪个 SO 触发的。ABT 用视图 `v_purchase_demands` 的 `cascade_from_product_name` 实现等价。

**Files:**
- Create: `abt-core/migrations/044_purchase_demand_view_cascade.sql`

- [ ] **Step 1: 更新视图**

创建 `abt-core/migrations/044_purchase_demand_view_cascade.sql`：

```sql
-- 044: 更新采购需求池视图，包含 BOM 级联来源信息
-- 参考 Odoo purchase_order_line.origin 字段追溯来源

CREATE OR REPLACE VIEW v_purchase_demands AS
SELECT
    d.id,
    d.demand_type,
    d.source_type,
    d.source_id AS order_id,
    d.source_line_id,
    d.product_id,
    d.acquire_channel,
    d.required_qty AS quantity,
    d.required_date,
    d.status AS demand_status,
    d.priority,
    d.target_doc_type,
    d.target_doc_id,
    d.cascade_from_product_id,
    d.remark,
    d.operator_id,
    d.created_at,
    so.doc_number AS order_no,
    p.pdt_name AS product_name,
    p.product_code,
    fp.pdt_name AS cascade_from_product_name,
    c.customer_name
FROM demands d
LEFT JOIN sales_orders so ON so.id = d.source_id AND d.source_type = 1
LEFT JOIN products p ON p.product_id = d.product_id
LEFT JOIN products fp ON fp.product_id = d.cascade_from_product_id
LEFT JOIN customers c ON c.customer_id = so.customer_id
WHERE d.acquire_channel = 2
  AND d.deleted_at IS NULL;
```

- [ ] **Step 2: 运行迁移**

Run: `psql $DATABASE_URL -f abt-core/migrations/044_purchase_demand_view_cascade.sql`

- [ ] **Step 3: 在 DemandSummary 中添加级联来源字段**

在 `abt-core/src/purchase/demand_handler/model.rs` 的 `DemandSummary`（约 line 19-33）中添加：

```rust
    /// BOM 级联来源成品名称（Odoo origin 等价）
    pub cascade_from_product_name: Option<String>,
```

- [ ] **Step 4: 运行 clippy 验证**

Run: `cargo clippy -p abt-core 2>&1 | head -20`

Expected: 无编译错误（`DemandSummary` 用 `sqlx::FromRow`，新列自动映射）。

- [ ] **Step 5: Commit**

```bash
git add abt-core/migrations/044_purchase_demand_view_cascade.sql abt-core/src/purchase/demand_handler/model.rs
git commit -m "feat(purchase): show BOM cascade origin in demand pool view (Odoo origin pattern)"
```

---

## Task 6: SO 详情页 Smart Button（参考 Odoo `oe_button_box`）

> **ERP 参考**：Odoo `sale_order_views.xml:411` `<div class="oe_button_box">` — 表单顶部一行统计按钮，计数为 0 时隐藏。每个按钮 = 图标 + 计数 + 标签，点击跳转关联单据列表。

**Files:**
- Modify: `abt-web/src/pages/sales_order_detail.rs`

- [ ] **Step 1: 在 handler 中查询关联需求数量**

在 `abt-web/src/pages/sales_order_detail.rs` 的 `get_order_detail` handler 中，加载完已有数据后、渲染 HTML 前，添加需求统计查询。

首先搜索文件中如何获取 `state` 和调用 service 的模式（每个 handler 不同，参照已有代码风格）。

在渲染主体 HTML 之前添加 smart button 区域。找到 order header 渲染之后、order lines 表格之前的位置，插入：

```rust
        // Smart Buttons — 参考 Odoo oe_button_box
        // 计数为 0 时隐藏（Odoo invisible="count == 0" 等价）
        div class="flex gap-3 mb-4" {
            // 自制需求数
            @if producing_count > 0 {
                a class="info-card flex items-center gap-2 px-4 py-2 hover:shadow-md transition-shadow"
                  href=(format!("/admin/mes/demand-pool?order_id={}", path.id))
                {
                    span class="text-2xl font-bold text-blue-600" { (producing_count) }
                    span class="text-sm text-gray-500" { "自制需求" }
                }
            }
            // 采购需求数
            @if purchasing_count > 0 {
                a class="info-card flex items-center gap-2 px-4 py-2 hover:shadow-md transition-shadow"
                  href=(format!("/admin/purchase/demand-pool?order_id={}", path.id))
                {
                    span class="text-2xl font-bold text-orange-600" { (purchasing_count) }
                    span class="text-sm text-gray-500" { "采购需求" }
                }
            }
            // BOM 展开需求数
            @if cascade_count > 0 {
                div class="info-card flex items-center gap-2 px-4 py-2" {
                    span class="text-2xl font-bold text-purple-600" { (cascade_count) }
                    span class="text-sm text-gray-500" { "BOM展开需求" }
                }
            }
        }
```

数据查询代码（在 handler 中，渲染之前）：

```rust
    // 关联需求统计
    let demands = state.demand_service()
        .find_by_source(&ctx, &state.pool, DocumentType::SalesOrder as i16, path.id)
        .await
        .unwrap_or_default();
    let producing_count = demands.iter().filter(|d| d.acquire_channel == 1).count();
    let purchasing_count = demands.iter().filter(|d| d.acquire_channel == 2).count();
    let cascade_count = demands.iter().filter(|d| d.demand_type == 2).count();
```

**注意**：需确认 `state.demand_service()` 的确切方法签名和返回类型。参照同文件中已有 service 调用模式。如果 `demand_service` 方法不存在于 `AppState`，需在 `abt-web/src/state.rs` 中添加。

- [ ] **Step 2: 运行 clippy 验证**

Run: `cargo clippy -p abt-web 2>&1 | head -30`

Expected: 无编译错误。

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/sales_order_detail.rs
git commit -m "feat(ui): add smart buttons for demand counts on SO detail (Odoo oe_button_box pattern)"
```

---

## Task 7: 履约工作台行状态着色（参考 Odoo decoration）

> **ERP 参考**：Odoo `mrp_production_views.xml:66-71`
> ```xml
> <field name="components_availability"
>     decoration-success="reservation_state == 'assigned'"
>     decoration-warning="...state in ('expected', 'available')"
>     decoration-danger="...state in ('late', 'unavailable')"/>
> ```
> 根据字段值动态着色：绿(success)/橙(warning)/红(danger)。

**Files:**
- Modify: `abt-web/src/pages/sales_order_detail.rs`

- [ ] **Step 1: 在履行计划行渲染中添加状态着色**

搜索 `sales_order_detail.rs` 中渲染 `FulfillmentPlanLine` 或 `shortage_qty` 的位置。

在每行渲染中，根据 `reserved_qty` 和 `shortage_qty` 添加 Odoo decoration 等价的条件 class：

```rust
    // Odoo decoration 等价：success(绿) / warning(橙) / danger(红)
    let (status_label, status_class) = if line.shortage_qty <= Decimal::ZERO {
        ("充足", "bg-green-100 text-green-700")
    } else if line.reserved_qty > Decimal::ZERO {
        ("部分缺货", "bg-yellow-100 text-yellow-700")
    } else {
        ("缺货", "bg-red-100 text-red-700")
    };
```

在行中添加 status pill 单元格：

```rust
    td {
        span class=(format!("status-pill {status_class}")) { (status_label) }
    }
```

- [ ] **Step 2: 运行 clippy 验证**

Run: `cargo clippy -p abt-web 2>&1 | head -20`

Expected: 无编译错误。

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/sales_order_detail.rs
git commit -m "feat(ui): color-code fulfillment lines by stock availability (Odoo decoration pattern)"
```

---

## Task 8: 设计文档同步

**Files:**
- Modify: `docs/uml-design/04-mes.html`

- [ ] **Step 1: 在 MES 设计文档中添加 BOM 级联说明**

在 `docs/uml-design/04-mes.html` 中找到需求流转相关章节，添加：

- 自制产品确认后，系统自动递归展开 BOM（参考 Odoo `_run_manufacture`）
- 原材料按 projected_qty（ERPNext 公式）扣减库存后，缺口生成 `demand_type=2` 采购需求
- projected_qty = actual + on_order_po + in_progress_wo − reserved
- 参考文档：`docs/erp-comparison-sales-to-mrp-purchase.md`

**注意**：设计文档变更需用户确认后修改。

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs: sync MES design doc with BOM cascade implementation"
```

---

## Task 9: 集成验证

- [ ] **Step 1: 全量 clippy**

Run: `cargo clippy 2>&1 | head -50`

Expected: 无错误。

- [ ] **Step 2: 全量测试**

Run: `cargo test -p abt-core 2>&1 | tail -20`

Expected: 全部通过。

- [ ] **Step 3: 手动验证 — SO 确认触发 BOM 级联**

1. 创建一个包含自制产品的 SO（产品 acquire_channel=1，有关联的已发布 BOM）
2. 确认 SO
3. 检查采购需求池是否出现 BOM 原材料采购需求
4. 检查 SO 详情页 Smart Button 统计
5. 检查履约工作台行着色

- [ ] **Step 4: 验证 projected_qty 正确性**

手动查询某个原材料的 projected_qty 各维度值，确认在途 PO 和在制 WO 被正确计算。

---

## Self-Review

### 对比文档 §5.2 设计要素参考矩阵覆盖检查

| 设计要素 | 推荐参考 | 实现 Task | 验证 |
|---|---|---|---|
| 整体架构 | Odoo StockRule | 保持 ABT Demand + Event 架构 | ✅ 不改架构，在 Demand 层加 BOM 级联 |
| BOM 级联 | Odoo `_run_manufacture` 递归 | Task 2 | ✅ `explode_recursive` 递归展开 |
| 库存可用量 | ERPNext `projected_qty` | Task 1 | ✅ `query_projected_qty` 四维公式 |
| 采购自动合并 | Odoo `_make_po_get_domain` | 已有 `find_material_aggregated` | ✅ 按物料聚合已有 |
| MTS 先查库存 | Odoo `mts_else_mto` | Task 4 Step 2 | ✅ projected_qty 够则跳过 |
| 需求去重 | Odoo `_make_mo_get_domain` | Task 4 Step 1 | ✅ `find_cascade_existing` |
| 需求→下游单据 | ABT 自有 | 不改 | ✅ |
| UI 行着色 | Odoo decoration | Task 7 | ✅ success/warning/danger |
| UI Smart Button | Odoo `oe_button_box` | Task 6 | ✅ count=0 时隐藏 |

### 新增 vs 上一版的差异

| 改进点 | 上一版 | 本版 |
|---|---|---|
| 库存查询 | `query_available`（仅 actual-reserved） | `query_projected_qty`（actual+ordered+planned-reserved） |
| 在途 PO 量 | 不考虑 | SQL JOIN purchase_order_items 计算未到货量 |
| 在制 WO 量 | 不考虑 | SQL JOIN work_orders 计算未完工量 |
| ERP 引用 | 模糊 | 每个 Task 标注具体 ERP + 源码位置 |
| projected_qty 结构 | 无 | `ProjectedQty` 分解四维 + 净值 |
| 去重查询 | 有 | 有，标注 Odoo `_make_mo_get_domain` 参考 |
