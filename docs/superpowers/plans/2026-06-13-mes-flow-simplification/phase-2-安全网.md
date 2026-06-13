# 阶段 2：安全网 + picking 模式

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 添加反下达能力（安全网）+ 引入产品级 `material_consumption_mode`，解锁 picking 模式的预留和领料单。

**Architecture:** 在 `WorkOrderService` 新增 `unrelease()` 方法。在 `ProductMeta` JSONB 字段中增加 `material_consumption_mode`。改造 `release()` 根据 mode 分流（picking → 预留 + 领料单，backflush → 跳过）。

**Tech Stack:** Rust + sqlx + async-trait

**前置:** 阶段 1 已上线并稳定

**验收:**
- unrelease 未开工工单 → 工单回 Draft，预留释放，领料单取消
- 产品切 picking → release 时 HARD 预留组件 + 领料单有明细行
- 产品切 backflush → 行为与阶段 1 一致

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 修改 | `abt-core/src/master_data/product/model.rs` | `ProductMeta` 增加 `material_consumption_mode` 字段 |
| 新增方法 | `abt-core/src/mes/work_order/service.rs` | `unrelease()` trait 方法签名 |
| 新增方法 | `abt-core/src/mes/work_order/implt.rs` | `unrelease()` 实现 |
| 修改 | `abt-core/src/mes/work_order/implt.rs` | `release()` 增加 picking/backflush 分流 |
| 修改 | `abt-core/src/wms/material_requisition/implt.rs` | `create_for_work_order()` 增加 BOM 快照明细行 |
| 新增方法 | `abt-core/src/mes/production_plan/repo.rs` | 更新 PlanItem 状态方法（unrelease 回滚用） |
| 修改 | `abt-core/src/mes/production_plan/implt.rs` | `release_to_work_orders()` 传入 routing/work_center |

---

### Task 1: ProductMeta 增加 material_consumption_mode

**Files:**
- Modify: `abt-core/src/master_data/product/model.rs` (lines 137-144, `ProductMeta` struct)

- [ ] **Step 1: 在 ProductMeta 中添加字段**

找到 `ProductMeta` struct：

```rust
pub struct ProductMeta {
    pub specification: String,
    pub old_code: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}
```

替换为：

```rust
/// 物料消耗策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MaterialConsumptionMode {
    /// 倒冲模式（默认）：完工时按 BOM 自动扣减原材料
    #[serde(rename = "backflush")]
    Backflush,
    /// 领料模式：release 时生成领料单，手动领料出库
    #[serde(rename = "picking")]
    Picking,
}

impl Default for MaterialConsumptionMode {
    fn default() -> Self {
        Self::Backflush
    }
}

pub struct ProductMeta {
    pub specification: String,
    /// acquire_channel 已迁移为 Product 独立列
    pub old_code: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    /// 物料消耗策略：backflush（默认）或 picking
    #[serde(default)]
    pub material_consumption_mode: MaterialConsumptionMode,
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 可能有一些 warning（下游代码适配），不应有 error（字段有 `#[serde(default)]`）

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/master_data/product/model.rs
git commit -m "feat(product): add material_consumption_mode to ProductMeta JSONB field"
```

---

### Task 2: WorkOrderService — 新增 unrelease() trait 方法

**Files:**
- Modify: `abt-core/src/mes/work_order/service.rs`

- [ ] **Step 1: 在 WorkOrderService trait 中添加 unrelease 方法**

在 `release()` 方法签名之后添加：

```rust
    /// 反下达工单：Released -> Draft
    /// 安全网操作：取消领料单、释放库存预留、删除批次和工序
    async fn unrelease(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -20`
Expected: 编译错误（未实现 trait 方法），下一步实现

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/work_order/service.rs
git commit -m "feat(work-order): add unrelease() to WorkOrderService trait"
```

---

### Task 3: WorkOrderServiceImpl — 实现 unrelease()

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: 在 implt.rs 中添加需要的 use**

```rust
use crate::wms::material_requisition::{new_material_requisition_service, service::MaterialRequisitionService};
```

（如果在阶段 1 已删除此 import，需要重新添加）

- [ ] **Step 2: 在 WorkOrderServiceImpl 的 `release()` 方法之后、`close()` 方法之前添加 `unrelease()` 实现**

```rust
    /// 反下达工单：Released -> Draft
    /// 安全网操作：仅在工单未开工时允许
    async fn unrelease(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        // 1. 加载工单，校验状态
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Released {
            return Err(DomainError::BusinessRule(
                "只有已下达状态的工单才能反下达".to_string(),
            ));
        }

        // 2. 校验未开工（所有批次 current_step == 0）
        let batches = ProductionBatchRepo::list_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let has_started = batches.iter().any(|b| b.current_step > 0);
        if has_started {
            return Err(DomainError::BusinessRule(
                "工单已开工，无法反下达".to_string(),
            ));
        }

        // 3. 取消领料单（如果存在）
        // 通过 document_link 查找关联的领料单，然后取消
        let links = new_document_link_service(self.pool.clone())
            .find_links(
                ctx, db,
                crate::shared::enums::DocumentType::WorkOrder,
                id,
            )
            .await?;

        for link in links {
            if link.target_type == crate::shared::enums::DocumentType::MaterialRequisition {
                // 取消领料单（忽略错误，可能已不存在）
                let _ = new_material_requisition_service(self.pool.clone())
                    .cancel(ctx, db, link.target_id).await;
            } else if link.source_type == crate::shared::enums::DocumentType::MaterialRequisition {
                let _ = new_material_requisition_service(self.pool.clone())
                    .cancel(ctx, db, link.source_id).await;
            }
        }

        // 4. 释放库存 HARD 预留（忽略错误，可能没有预留）
        let _ = new_inventory_reservation_service(self.pool.clone())
            .cancel_by_source(ctx, db, DocumentType::WorkOrder, id).await;

        // 5. 删除 ProductionBatch（WHERE work_order_id = id）
        sqlx::query("DELETE FROM production_batches WHERE work_order_id = $1")
            .bind(id)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 6. 删除 WorkOrderRouting（WHERE work_order_id = id）
        sqlx::query("DELETE FROM work_order_routings WHERE work_order_id = $1")
            .bind(id)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 7. 清除 bom_snapshot_id（快照记录保留）
        sqlx::query("UPDATE work_orders SET bom_snapshot_id = NULL, routing_id = NULL, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 8. 工单状态 → Draft
        let updated = WorkOrderRepo::update_status_with_version(
            &mut *db,
            id,
            WorkOrderStatus::Draft,
            expected_version,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        // 9. 回滚关联 PlanItem 状态：Released → Planned
        if let Some(plan_item_id) = work_order.plan_item_id {
            let _ = sqlx::query(
                "UPDATE production_plan_items SET status = 1 WHERE id = $1 AND status = 2",
            )
            .bind(plan_item_id)
            .execute(&mut *db)
            .await;
        }

        // 10. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }
```

- [ ] **Step 3: 检查 document_link 服务是否有所需方法**

如果 `new_document_link_service()` 没有 `find_links()` 方法，需要用 SQL 替代：

```rust
        // 3. 取消领料单（如果存在）
        let requisition_ids: Vec<i64> = sqlx::query_scalar(
            r#"SELECT source_id FROM document_links
               WHERE target_type = 17 AND target_id = $1
               UNION
               SELECT target_id FROM document_links
               WHERE source_type = 17 AND source_id = $1"#,
        )
        .bind(id)
        .fetch_all(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        for req_id in requisition_ids {
            let _ = new_material_requisition_service(self.pool.clone())
                .cancel(ctx, db, req_id).await;
        }
```

注意：`DocumentType::MaterialRequisition` 的值需要确认。检查 `shared/enums` 中的值，假设 MaterialRequisition = 17。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs
git commit -m "feat(work-order): implement unrelease() — reverse release with safety checks

- Verify work order status and no started batches
- Cancel material requisitions
- Release inventory reservations
- Delete production batches and routings
- Clear bom_snapshot_id and routing_id
- Roll back PlanItem status"
```

---

### Task 4: release() 增加 picking/backflush 分流

在阶段 1 的 backflush-only 路径基础上，增加按产品 `material_consumption_mode` 分流。

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs` (release() 方法中的 step 7)

- [ ] **Step 1: 在 release() 中替换 step 7（backflush 注释）**

找到 release() 中的这段注释和代码：

```rust
        // 7. backflush 模式：不预留、不创建领料单
        // （阶段 2 引入 picking 模式时在此处分流）
```

替换为完整的分流逻辑：

```rust
        // 7. 根据产品 material_consumption_mode 分流
        let consumption_mode = product.meta.material_consumption_mode;

        match consumption_mode {
            crate::master_data::product::model::MaterialConsumptionMode::Picking => {
                // picking 模式：HARD 预留组件 + 生成领料单明细行
                if let Some(snap_id) = bom_snapshot_id {
                    // 从 BOM 快照展开叶子节点 → 预留每个组件
                    let snapshot_opt = new_bom_query_service(self.pool.clone())
                        .get_snapshot_by_id(ctx, db, snap_id).await?;

                    if let Some(snapshot) = snapshot_opt {
                        let all_nodes = &snapshot.bom_detail.nodes;
                        let parent_ids: std::collections::HashSet<i64> =
                            all_nodes.iter().map(|n| n.parent_id).collect();
                        let leaf_nodes: Vec<&crate::master_data::bom::model::BomNode> = all_nodes
                            .iter()
                            .filter(|n| !parent_ids.contains(&n.id))
                            .collect();

                        if !leaf_nodes.is_empty() {
                            let warehouse_id = crate::wms::backflush::implt::resolve_warehouse_id_static(&self.pool, db).await?;

                            // HARD 预留每个组件
                            let reserve_requests: Vec<ReserveRequest> = leaf_nodes.iter().map(|node| {
                                ReserveRequest {
                                    product_id: node.product_id,
                                    warehouse_id,
                                    reserved_qty: node.quantity * work_order.planned_qty,
                                    reservation_type: ReservationType::Hard,
                                    source_type: DocumentType::WorkOrder,
                                    source_id: id,
                                    source_line_id: None,
                                    priority: 0,
                                    expires_at: None,
                                }
                            }).collect();

                            new_inventory_reservation_service(self.pool.clone())
                                .reserve(ctx, db, reserve_requests)
                                .await?;
                        }
                    }

                    // 创建领料单（含明细行）
                    new_material_requisition_service(self.pool.clone())
                        .create_for_work_order(ctx, db, id).await?;
                }
            }
            crate::master_data::product::model::MaterialConsumptionMode::Backflush => {
                // backflush 模式：不预留、不创建领料单
                // 倒冲在完工时按实际量自动扣减
            }
        }
```

- [ ] **Step 2: 确保需要的 use 存在**

确认 `implt.rs` 顶部有这些 import：

```rust
use crate::shared::inventory_reservation::{new_inventory_reservation_service, model::ReserveRequest, service::InventoryReservationService};
use crate::shared::enums::ReservationType;
use crate::wms::material_requisition::{new_material_requisition_service, service::MaterialRequisitionService};
```

- [ ] **Step 3: 导出 resolve_warehouse_id 供 release() 调用**

如果 `resolve_warehouse_id` 是 backflush implt 中的私有函数，需要改为 `pub` 或提取为共享函数。

方案 A（推荐）：在 `backflush/mod.rs` 导出为公共函数：
```rust
pub use implt::resolve_warehouse_id as resolve_warehouse_id_static;
```

方案 B：将 `resolve_warehouse_id` 提取到 `shared/` 中作为共享工具函数。

选择方案 A 的前提：确认 `abt-core/src/wms/backflush/mod.rs` 的导出方式。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs abt-core/src/wms/backflush/
git commit -m "feat(mes): release() picking/backflush split by product material_consumption_mode

- picking mode: HARD reserve all BOM components + create requisition with line items
- backflush mode: no reservation, no requisition (default, backward compatible)"
```

---

### Task 5: 领料单明细行生成（picking 模式）

**Files:**
- Modify: `abt-core/src/wms/material_requisition/implt.rs` (lines 36-77, `create_for_work_order()`)

- [ ] **Step 1: 重写 create_for_work_order()**

完整替换 `create_for_work_order()` 方法：

```rust
    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::MaterialRequisition)
            .await
            .unwrap_or_else(|_| format!("MR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let requisition_date = chrono::Local::now().date_naive();

        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id).await?;

        // 确定仓库（V1：使用工单的 work_center_id 或回退策略）
        let warehouse_id = wo.work_center_id.unwrap_or(0);

        let requisition = MaterialRequisitionRepo::insert(
            &mut *db,
            &doc_number,
            work_order_id,
            requisition_date,
            warehouse_id,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 从 BOM 快照展开组件 → 生成领料单明细行
        if let Some(snapshot_id) = wo.bom_snapshot_id {
            let snapshot_opt = new_bom_query_service(self.pool.clone())
                .get_snapshot_by_id(ctx, db, snapshot_id).await?;

            if let Some(snapshot) = snapshot_opt {
                let all_nodes = &snapshot.bom_detail.nodes;
                let parent_ids: std::collections::HashSet<i64> =
                    all_nodes.iter().map(|n| n.parent_id).collect();
                let leaf_nodes: Vec<&crate::master_data::bom::model::BomNode> = all_nodes
                    .iter()
                    .filter(|n| !parent_ids.contains(&n.id))
                    .collect();

                for node in &leaf_nodes {
                    let required_qty = node.quantity * wo.planned_qty;
                    MaterialRequisitionRepo::insert_item(
                        &mut *db,
                        requisition.id,
                        node.product_id,
                        required_qty,
                    )
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }

        new_document_link_service(self.pool.clone())
        .create_links(
            ctx, db,
            vec![LinkRequest {
                source_type: DocumentType::MaterialRequisition,
                source_id: requisition.id,
                target_type: DocumentType::WorkOrder,
                target_id: work_order_id,
                link_type: LinkType::Fulfills,
            }],
        )
        .await?;

        Ok(requisition.id)
    }
```

- [ ] **Step 2: 添加需要的 use**

在 `material_requisition/implt.rs` 顶部添加：

```rust
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/wms/material_requisition/implt.rs
git commit -m "feat(requisition): generate line items from BOM snapshot in create_for_work_order()

- Expand BOM snapshot leaf nodes into requisition items
- Each item: product_id + required_qty (node.quantity × planned_qty)"
```

---

### Task 6: release_to_work_orders() 传入 routing/work_center 信息

**Files:**
- Modify: `abt-core/src/mes/production_plan/implt.rs` (lines 121-135)

- [ ] **Step 1: 在 CreateWorkOrderReq 中传入 item 的 routing_id 和 work_center_id**

找到 CreateWorkOrderReq 构建处，将 `routing_id: None` 和 `work_center_id: None` 替换为：

```rust
                CreateWorkOrderReq {
                    plan_item_id: Some(item.id),
                    product_id: item.product_id,
                    bom_snapshot_id: None, // release() 中动态创建
                    routing_id: item.routing_id,
                    planned_qty: item.planned_qty,
                    scheduled_start,
                    scheduled_end,
                    work_center_id: item.work_center_id,
                    sales_order_id: item.sales_order_id,
                    remark: None,
                },
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_plan/implt.rs
git commit -m "fix(mes): pass routing_id, work_center_id from PlanItem to WorkOrder"
```

---

### Task 7: 验证阶段 2

- [ ] **Step 1: 全量 clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: 无 error

- [ ] **Step 2: 运行测试**

Run: `cargo test -p abt-core 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: 最终 commit**

```bash
git add -A
git commit -m "feat(mes): phase 2 complete — unrelease + picking mode + requisition line items"
```
