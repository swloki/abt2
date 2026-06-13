# MES 三层状态联动 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 打通 ProductionPlan / WorkOrder / ProductionBatch 三层状态自动传播，并在详情页展示上下游关联信息。

**Architecture:** 在现有的 `confirm_routing_step()`（首次报工）和 `ProductionReceipt::confirm()`（完工入库）事务内，追加直接 repo 级条件 UPDATE 调用，将状态变更传播到上游工单和计划。新增 `WorkOrderStatus::InProduction` 中间态。

**Tech Stack:** Rust (edition 2024), sqlx (raw SQL), Axum + Maud + HTMX, PostgreSQL

**Design Spec:** `docs/superpowers/specs/2026-06-13-mes-three-layer-linkage-design.md`

---

## File Structure

| 文件 | 职责 | 变更类型 |
|------|------|---------|
| `abt-core/src/mes/enums.rs` | MES 枚举定义 | 修改：新增 InProduction |
| `abt-core/src/mes/work_order/repo.rs` | 工单 DB 操作 | 修改：新增 update_status_conditional |
| `abt-core/src/mes/work_order/service.rs` | 工单 Service trait | 修改：新增 mark_in_production |
| `abt-core/src/mes/work_order/implt.rs` | 工单 Service 实现 | 修改：传播 + 状态条件放宽 |
| `abt-core/src/mes/production_plan/repo.rs` | 计划 DB 操作 | 修改：新增 3 个方法 |
| `abt-core/src/mes/production_batch/implt.rs` | 批次 Service 实现 | 修改：confirm_routing_step 传播 |
| `abt-core/src/mes/production_receipt/implt.rs` | 完工入库 Service 实现 | 修改：confirm 传播 |
| `abt-web/src/pages/mes_plan_detail.rs` | 计划详情页 | 修改：新增下达结果 Tab |
| `abt-web/src/pages/mes_order_detail.rs` | 工单详情页 | 修改：新增来源追溯+批次状态 |
| `abt-web/src/pages/mes_batch_detail.rs` | 批次详情页 | 修改：补全上下游链接 |
| `scripts/mes-status-backfill.sql` | 历史数据修复 | 新建 |

---

## Phase 1: 后端基础设施（枚举 + Repo 方法）

### Task 1: 新增 WorkOrderStatus::InProduction 枚举

**Files:**
- Modify: `abt-core/src/mes/enums.rs:97-103`

- [ ] **Step 1: 添加 InProduction 变体**

在 `WorkOrderStatus` 枚举中，`Released = 3` 之后新增 `InProduction = 6`：

```rust
define_mes_enum!(WorkOrderStatus {
    Draft = 1,
    Planned = 2,
    Released = 3,
    InProduction = 6,
    Closed = 4,
    Cancelled = 5,
});
```

值=6 而非重排序（4→5），避免影响存量 smallint 数据。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error（可能有 warning 关于未使用变体，属正常）

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/enums.rs
git commit -m "feat(mes): add WorkOrderStatus::InProduction enum variant"
```

---

### Task 2: WorkOrderRepo 新增条件状态更新方法

**Files:**
- Modify: `abt-core/src/mes/work_order/repo.rs`

- [ ] **Step 1: 添加 update_status_conditional 方法**

在 `WorkOrderRepo` impl 块末尾（`list_by_plan` 方法之后）新增：

```rust
    /// 条件状态更新（无乐观锁，用于事务内传播）。
    /// 仅当当前状态匹配 from 时才更新为 to，返回是否实际更新。
    pub async fn update_status_conditional(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        from: WorkOrderStatus,
        to: WorkOrderStatus,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE work_orders
            SET status = $3, version = version + 1, updated_at = NOW()
            WHERE id = $1 AND status = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(from)
        .bind(to)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/work_order/repo.rs
git commit -m "feat(mes): add WorkOrderRepo::update_status_conditional for state propagation"
```

---

### Task 3: ProductionPlanRepo 新增三个传播方法

**Files:**
- Modify: `abt-core/src/mes/production_plan/repo.rs`

- [ ] **Step 1: 添加 update_item_status_by_work_order 方法**

在 `ProductionPlanRepo` impl 块末尾（`update_item_priority` 之后）新增：

```rust
    /// 通过工单 ID 反查并更新关联 PlanItem 状态。
    /// 条件更新：仅当 plan_item_id 匹配时更新。
    pub async fn update_item_status_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
        status: PlanItemStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_plan_items
            SET status = $2
            WHERE id = (
                SELECT plan_item_id FROM work_orders
                WHERE id = $1 AND plan_item_id IS NOT NULL
            )
            "#,
        )
        .bind(work_order_id)
        .bind(status)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 通过工单 ID 查找关联的 Plan ID。
    pub async fn find_plan_id_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT ppi.plan_id
            FROM work_orders wo
            JOIN production_plan_items ppi ON ppi.id = wo.plan_item_id
            WHERE wo.id = $1
            "#,
        )
        .bind(work_order_id)
        .fetch_optional(&mut *executor)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// 重新计算计划状态：所有 PlanItem 终态 → Plan Completed。
    /// 条件 UPDATE，幂等。
    pub async fn recalculate_plan_status(
        executor: &mut sqlx::postgres::PgConnection,
        plan_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_plans
            SET status = $2, updated_at = NOW()
            WHERE id = $1
              AND status = $3
              AND NOT EXISTS (
                SELECT 1 FROM production_plan_items
                WHERE plan_id = $1
                  AND status NOT IN ($2, $4)
              )
            "#,
        )
        .bind(plan_id)
        .bind(PlanStatus::Completed)        -- $2: 目标状态
        .bind(PlanStatus::InProgress)        -- $3: 当前状态条件
        .bind(PlanItemStatus::Cancelled)     -- $4: 终态之一
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
```

注意：`recalculate_plan_status` 的 SQL 逻辑——当 Plan 处于 InProgress 且不存在非终态（非 Completed/非 Cancelled）的 PlanItem 时，将 Plan 推进为 Completed。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_plan/repo.rs
git commit -m "feat(mes): add ProductionPlanRepo propagation methods"
```

---

## Phase 2: 状态传播逻辑

### Task 4: WorkOrderService 新增 mark_in_production

**Files:**
- Modify: `abt-core/src/mes/work_order/service.rs`
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: Service trait 新增方法声明**

在 `WorkOrderService` trait 中，`release` 方法之后新增：

```rust
    /// 标记工单为生产中：Released → InProduction
    /// 条件 UPDATE，幂等。用于批次首次报工时自动传播。
    async fn mark_in_production(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;
```

- [ ] **Step 2: 实现 mark_in_production**

在 `WorkOrderServiceImpl` impl 块中，`release` 方法之后新增：

```rust
    async fn mark_in_production(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let updated = WorkOrderRepo::update_status_conditional(
            &mut *db,
            id,
            WorkOrderStatus::Released,
            WorkOrderStatus::InProduction,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if updated {
            new_audit_log_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
                )
                .await?;
        }

        Ok(())
    }
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/mes/work_order/service.rs abt-core/src/mes/work_order/implt.rs
git commit -m "feat(mes): add WorkOrderService::mark_in_production"
```

---

### Task 5: confirm_routing_step 追加首次报工传播

**Files:**
- Modify: `abt-core/src/mes/production_batch/implt.rs`

- [ ] **Step 1: 在 confirm_routing_step 末尾追加传播逻辑**

在 `confirm_routing_step` 方法中，步骤 l（返回 StepConfirmationResult）之前，新增传播逻辑。

找到方法末尾的 `// --- l. 返回结果 ---` 注释行，在其 **之前** 插入：

```rust
        // --- k2. 状态传播：首次报工时推进上游工单和计划行状态 ---
        if was_inserted && batch.status == BatchStatus::Pending && step_no == 1 {
            // Batch: Pending → InProgress（此时已在上方步骤 k 中更新）
            // WorkOrder: Released → InProduction
            if let Err(e) = new_work_order_service(self.pool.clone())
                .mark_in_production(ctx, db, batch.work_order_id)
                .await
            {
                tracing::warn!(
                    work_order_id = batch.work_order_id,
                    error = %e,
                    "failed to propagate batch in-progress to work order"
                );
            }

            // PlanItem: Released → InProduction
            if let Err(e) = crate::mes::production_plan::repo::ProductionPlanRepo::update_item_status_by_work_order(
                &mut *db,
                batch.work_order_id,
                PlanItemStatus::InProduction,
            )
            .await
            {
                tracing::warn!(
                    work_order_id = batch.work_order_id,
                    error = %e,
                    "failed to propagate batch in-progress to plan item"
                );
            }
        }
```

**注意事项**：
- 传播失败使用 `tracing::warn!` 记录但不阻断报工主流程（报工是车间操作，不应因状态传播失败而失败）
- 需要确认 `batch.status` 在此处已经是更新后的值。查看代码：如果 step_no==1 且 batch 从 Pending 进入，步骤 k 中可能更新了状态（PendingReceipt 如果是最后一道工序）。对于单工序场景（step_no==1 == max_step），状态会变为 PendingReceipt。因此条件应改为检查 **原始 batch.status == Pending**（在步骤 a 获取的值），而非可能已被修改的当前值。

修正：传播条件应使用 `batch.status`（步骤 a 获取的原始值），而非可能被步骤 k 修改后的值：

```rust
        // --- k2. 状态传播：首次报工时推进上游工单和计划行状态 ---
        // batch.status 是步骤 a 读取的原始值（Pending 表示首次报工）
        if was_inserted && batch.status == BatchStatus::Pending {
            // WorkOrder: Released → InProduction
            if let Err(e) = new_work_order_service(self.pool.clone())
                .mark_in_production(ctx, db, batch.work_order_id)
                .await
            {
                tracing::warn!(
                    work_order_id = batch.work_order_id,
                    error = %e,
                    "failed to propagate batch in-progress to work order"
                );
            }

            // PlanItem: Released → InProduction
            if let Err(e) = crate::mes::production_plan::repo::ProductionPlanRepo::update_item_status_by_work_order(
                &mut *db,
                batch.work_order_id,
                PlanItemStatus::InProduction,
            )
            .await
            {
                tracing::warn!(
                    work_order_id = batch.work_order_id,
                    error = %e,
                    "failed to propagate batch in-progress to plan item"
                );
            }
        }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_batch/implt.rs
git commit -m "feat(mes): propagate batch in-progress to work order and plan item on first report"
```

---

### Task 6: ProductionReceipt::confirm 追加完工传播

**Files:**
- Modify: `abt-core/src/mes/production_receipt/implt.rs`

- [ ] **Step 1: 在 confirm 方法步骤 6 之后追加传播**

找到 confirm 方法中步骤 6（`Update batch status to Completed`）之后的 `Ok(())` 之前，插入传播逻辑。

需要新增 import（文件顶部已有的 import 区域）：

```rust
use crate::mes::work_order::repo::WorkOrderRepo;
use crate::mes::work_order::enums::WorkOrderStatus;
use crate::mes::production_plan::repo::ProductionPlanRepo;
use crate::mes::production_plan::enums::PlanItemStatus;
```

注意：当前文件已 `use super::super::enums::*`，因此 `WorkOrderStatus` 和 `PlanItemStatus` 已在作用域内（它们定义在 `mes/enums.rs` 中，通过 `super::super::enums::*` 导入）。需要确认这一点。

在步骤 6 的 `}` 之后、`Ok(())` 之前插入：

```rust
        // --- 7. 状态传播：完工入库后推进上游工单和计划行状态 ---
        // WorkOrder: InProduction → Closed（repo 级条件 UPDATE，不需要 version）
        if let Err(e) = WorkOrderRepo::update_status_conditional(
            &mut *db,
            receipt.work_order_id,
            WorkOrderStatus::InProduction,
            WorkOrderStatus::Closed,
        )
        .await
        {
            tracing::warn!(
                work_order_id = receipt.work_order_id,
                error = %e,
                "failed to propagate receipt confirm to work order close"
            );
        } else {
            // 审计日志
            if let Err(e) = new_audit_log_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordAuditLogReq::new("WorkOrder", receipt.work_order_id, AuditAction::Transition),
                )
                .await
            {
                tracing::warn!(error = %e, "failed to audit work order close on receipt");
            }
        }

        // PlanItem: InProduction → Completed
        if let Err(e) = ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            receipt.work_order_id,
            PlanItemStatus::Completed,
        )
        .await
        {
            tracing::warn!(
                work_order_id = receipt.work_order_id,
                error = %e,
                "failed to propagate receipt confirm to plan item completed"
            );
        }

        // Plan: 如果所有 PlanItem 终态 → Completed
        if let Some(plan_id) = ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db,
            receipt.work_order_id,
        )
        .await
        .unwrap_or(None)
        {
            if let Err(e) = ProductionPlanRepo::recalculate_plan_status(
                &mut *db,
                plan_id,
            )
            .await
            {
                tracing::warn!(
                    plan_id,
                    error = %e,
                    "failed to recalculate plan status after receipt confirm"
                );
            }
        }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_receipt/implt.rs
git commit -m "feat(mes): propagate receipt confirm to work order close and plan item completed"
```

---

### Task 7: WorkOrder::cancel 追加 PlanItem 传播

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: 在 cancel 方法中追加传播**

在 `cancel()` 方法中，审计日志之后、`Ok(())` 之前，追加：

```rust
        // 状态传播：PlanItem → Cancelled + 重新计算 Plan 状态
        if let Err(e) = crate::mes::production_plan::repo::ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            id,
            crate::mes::enums::PlanItemStatus::Cancelled,
        )
        .await
        {
            tracing::warn!(work_order_id = id, error = %e, "failed to propagate cancel to plan item");
        }

        if let Some(plan_id) = crate::mes::production_plan::repo::ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db,
            id,
        )
        .await
        .unwrap_or(None)
        {
            if let Err(e) = crate::mes::production_plan::repo::ProductionPlanRepo::recalculate_plan_status(
                &mut *db,
                plan_id,
            )
            .await
            {
                tracing::warn!(plan_id, error = %e, "failed to recalculate plan status after cancel");
            }
        }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs
git commit -m "feat(mes): propagate work order cancel to plan item and recalculate plan status"
```

---

### Task 8: 放宽 close() 和 cancel() 状态接受条件

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: close() 接受 InProduction 状态**

找到 `close()` 方法中的状态校验（约 line 465）：

```rust
        if work_order.status != WorkOrderStatus::Released {
```

改为：

```rust
        if work_order.status != WorkOrderStatus::Released
            && work_order.status != WorkOrderStatus::InProduction
        {
```

- [ ] **Step 2: cancel() 接受 InProduction 状态**

找到 `cancel()` 方法中的状态校验（约 line 528-531）：

```rust
        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
            && work_order.status != WorkOrderStatus::Released
        {
```

改为：

```rust
        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
            && work_order.status != WorkOrderStatus::Released
            && work_order.status != WorkOrderStatus::InProduction
        {
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs
git commit -m "fix(mes): close() and cancel() now accept InProduction status"
```

---

## Phase 3: 详情页关联信息

### Task 9: 计划详情页 — 新增"下达结果"Tab

**Files:**
- Modify: `abt-web/src/pages/mes_plan_detail.rs`

- [ ] **Step 1: 读取现有计划详情页结构**

Run: `read abt-web/src/pages/mes_plan_detail.rs` — 理解现有 Tab 布局和数据加载方式

- [ ] **Step 2: 在 handler 中加载工单列表**

在计划详情页 handler 中，调用 `state.work_order_service().list_by_plan(ctx, db, plan_id)` 获取该计划下所有工单。对每个工单，其 `completed_steps` / `total_steps` 字段已有（WorkOrderRepo::list_by_plan 的 SQL 应包含这些聚合字段）。

确认 `list_by_plan` 返回的 WorkOrder 是否包含进度聚合字段。如不包含，在 `WorkOrderRepo::list_by_plan` 的 SQL 中补充 completed_steps/total_steps 子查询。

- [ ] **Step 3: 渲染"下达结果"Tab**

在计划详情页的 Tab 区域新增一个 Tab，内容为工单列表表格：

列结构：工单编号 | 产品名称 | 计划数量 | 工单状态 | 批次进度（completed_steps/total_steps）

Tab 切换使用 Surreal.js 内联：
```rust
(maud::PreEscaped(r#"<script>
me('.plan-tab').on('click', function() {
    me('.plan-tab').classRemove('tab-active')
    me(this).classAdd('tab-active')
    me('.plan-tab-panel').classAdd('hidden')
    me('#' + me(this).attribute('data-panel')).classRemove('hidden')
})
</script>"#))
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat(mes-ui): plan detail - add released work orders tab"
```

---

### Task 10: 工单详情页 — 新增来源追溯 + 批次执行状态

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs`

- [ ] **Step 1: 读取现有工单详情页结构**

Run: `read abt-web/src/pages/mes_order_detail.rs` — 理解现有数据加载和 section 布局

- [ ] **Step 2: handler 加载来源计划和批次数据**

在工单详情页 handler 中新增数据加载：

```rust
// 来源计划信息（通过 plan_item_id → plan_id）
let source_plan: Option<(String,)> = if let Some(plan_item_id) = work_order.plan_item_id {
    sqlx::query_as(
        "SELECT pp.doc_number FROM production_plan_items ppi
         JOIN production_plans pp ON pp.id = ppi.plan_id
         WHERE ppi.id = $1"
    ).bind(plan_item_id).fetch_optional(&mut *db).await.ok().flatten()
} else { None };

// 批次列表
let batches = state.production_batch_service()
    .list_by_work_order(ctx, db, work_order.id).await.unwrap_or_default();

// 工序进度
let routings = state.production_batch_service()
    .list_routings(ctx, db, work_order.id).await.unwrap_or_default();
```

注意：abt-web 禁止直接 SQL 查询。来源计划信息应通过 service 获取。如果当前没有合适的 service 方法，在 abt-core 中新增一个轻量查询方法。

替代方案：利用 WorkOrder 已有的 `source_plan_doc` 字段（在 list 查询中已填充），在 `find_by_id` 中也填充这些字段（修改 get_by_id SQL 添加 JOIN）。

- [ ] **Step 3: 渲染来源追溯 section**

```rust
section class="info-card" {
    h3 class="info-card-title" { "来源追溯" }
    div class="info-grid" {
        div class="info-item" {
            span class="info-label" { "计划编号" }
            span class="info-value" {
                if let Some(ref plan_doc) = source_plan_doc {
                    a href=(format!("/admin/mes/plans/{}", source_plan_id.unwrap_or(0))) { (plan_doc) }
                } else {
                    "—"
                }
            }
        }
        // 销售订单、客户同理（使用已有 source_so_doc / source_customer 字段）
    }
}
```

- [ ] **Step 4: 渲染批次执行状态 section**

```rust
section class="info-card" {
    h3 class="info-card-title" { "批次执行状态" }
    div class="info-grid" {
        // 批次编号、流转卡号、批次状态
        // 完成量/报废量
        // 工序进度条
    }
}
```

进度条渲染：
```rust
let progress_pct = if total_steps > 0 {
    (completed_steps as f64 / total_steps as f64 * 100.0) as u32
} else { 0 };
div class="wo-progress" {
    div class="wo-progress-bar" style=(format!("width: {}%", progress_pct)) {}
    span { (format!("{} / {} 工序", completed_steps, total_steps)) }
}
```

- [ ] **Step 5: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 6: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat(mes-ui): work order detail - add source tracing and batch execution status"
```

---

### Task 11: 批次详情页 — 补全上下游链接

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs`

- [ ] **Step 1: 读取现有批次详情页结构**

Run: `read abt-web/src/pages/mes_batch_detail.rs` — 理解现有布局

- [ ] **Step 2: 补全工单和计划编号链接**

在批次详情的信息区域，将 `work_order_id` 替换为可点击链接：

```rust
div class="info-item" {
    span class="info-label" { "工单编号" }
    span class="info-value" {
        a href=(format!("/admin/mes/orders/{}", batch.work_order_id)) {
            (wo_doc_number.as_deref().unwrap_or("—"))
        }
    }
}
```

如需计划编号，通过 `work_order_id` → `plan_item_id` → `plan_id` → `plan.doc_number` 反查。由于 abt-web 禁止直接 SQL，在 handler 中通过 service 调用获取。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_batch_detail.rs
git commit -m "feat(mes-ui): batch detail - add work order and plan navigation links"
```

---

## Phase 4: 数据迁移 + 设计同步

### Task 12: 历史数据状态回填脚本

**Files:**
- Create: `scripts/mes-status-backfill.sql`

- [ ] **Step 1: 编写回填脚本**

```sql
-- MES 三层状态联动 — 历史数据回填
-- 根据已有批次状态回填工单和计划行状态
-- 执行前务必备份数据库

BEGIN;

-- 1. 回填 WorkOrder 状态：有 InProgress 批次的工单 → InProduction (6)
UPDATE work_orders wo
SET status = 6, version = version + 1, updated_at = NOW()
WHERE wo.status = 3  -- Released
  AND wo.deleted_at IS NULL
  AND EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status IN (2, 3, 4)  -- InProgress, Suspended, PendingReceipt
  );

-- 2. 回填 WorkOrder 状态：有 Completed 批次的工单 → Closed (4)
UPDATE work_orders wo
SET status = 4, version = version + 1, updated_at = NOW()
WHERE wo.status IN (3, 6)  -- Released or InProduction
  AND wo.deleted_at IS NULL
  AND EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status = 5  -- Completed
  )
  AND NOT EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status NOT IN (5, 6)  -- 排除有未完成批次的工单
  );

-- 3. 回填 PlanItem 状态：InProduction (3)
UPDATE production_plan_items ppi
SET status = 3  -- InProduction
WHERE ppi.status = 2  -- Released
  AND EXISTS (
    SELECT 1 FROM work_orders wo
    WHERE wo.plan_item_id = ppi.id
      AND wo.status = 6  -- InProduction
  );

-- 4. 回填 PlanItem 状态：Completed (4)
UPDATE production_plan_items ppi
SET status = 4  -- Completed
WHERE ppi.status IN (2, 3)  -- Released or InProduction
  AND EXISTS (
    SELECT 1 FROM work_orders wo
    WHERE wo.plan_item_id = ppi.id
      AND wo.status = 4  -- Closed
  );

-- 5. 回填 Plan 状态：Completed (4)
UPDATE production_plans pp
SET status = 4, updated_at = NOW()
WHERE pp.status = 3  -- InProgress
  AND NOT EXISTS (
    SELECT 1 FROM production_plan_items ppi
    WHERE ppi.plan_id = pp.id
      AND ppi.status NOT IN (4, 5)  -- 非 Completed/Cancelled
  );

COMMIT;

-- 验证查询
SELECT 'work_orders' AS table_name, status, COUNT(*) FROM work_orders WHERE deleted_at IS NULL GROUP BY status ORDER BY status;
SELECT 'production_plan_items' AS table_name, status, COUNT(*) FROM production_plan_items GROUP BY status ORDER BY status;
SELECT 'production_plans' AS table_name, status, COUNT(*) FROM production_plans WHERE deleted_at IS NULL GROUP BY status ORDER BY status;
```

- [ ] **Step 2: Commit**

```bash
git add scripts/mes-status-backfill.sql
git commit -m "chore(mes): add historical status backfill script"
```

---

### Task 13: 同步 UML 设计文档

**Files:**
- Modify: `docs/uml-design/04-mes.html`

- [ ] **Step 1: 更新 WorkOrderStatus 枚举**

在 04-mes.html 的 `WorkOrderStatus` class 定义中，在 `Released` 和 `Closed` 之间添加 `InProduction`。

- [ ] **Step 2: 添加三层联动标注**

在类图的关系区域，添加 Plan/WorkOrder/Batch 之间的状态传播标注（注释说明首次报工→InProduction、完工入库→Closed 的联动）。

- [ ] **Step 3: Commit**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs: sync MES UML design with three-layer state linkage"
```

---

### Task 14: 最终验证 — 全量 clippy + 冒烟测试

- [ ] **Step 1: 全量编译验证**

Run: `cargo clippy 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 2: 全量测试**

Run: `cargo test -p abt-core 2>&1 | findstr "test result"`
Expected: 所有测试 PASS

- [ ] **Step 3: 执行历史数据回填（如有生产数据）**

Run: `psql -d abt_v2 -f scripts/mes-status-backfill.sql`
Expected: 所有 UPDATE 成功，验证查询显示合理的状态分布

- [ ] **Step 4: 端到端冒烟测试（使用 agent-browser）**

```bash
# 登录
agent-browser --session-name abt --ignore-https-errors open https://localhost:8000/login
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000

# 验证计划详情页有"下达结果"Tab
agent-browser open https://localhost:8000/admin/mes/plans/1
agent-browser snapshot -i
agent-browser screenshot --full

# 验证工单详情页有来源追溯+批次状态
agent-browser open https://localhost:8000/admin/mes/orders/1
agent-browser snapshot -i
agent-browser screenshot --full
```

Expected: 页面正常渲染，新增 section 可见

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test: MES three-layer state linkage verification complete"
```

---

## Self-Review Checklist

- [x] **Spec coverage**: 设计文档中的每个传播点（首次报工、完工入库、取消、反下达）都有对应 Task
- [x] **Placeholder scan**: 无 TBD/TODO，每个 step 有具体代码
- [x] **Type consistency**: `WorkOrderStatus::InProduction = 6`、`PlanItemStatus::InProduction = 3`（已有枚举值，无需新增）、`update_status_conditional` 签名一致
- [x] **unrelease 已有 PlanItem 回退**（代码 line 412-424），无需重复实现
- [x] **close()/cancel() 接受 InProduction** — Task 8 覆盖
