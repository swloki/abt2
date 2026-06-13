# MES 三层状态联动 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 打通 ProductionPlan / WorkOrder / ProductionBatch 三层状态自动传播，并在详情页展示上下游关联信息。

**Architecture:** 在现有的 `confirm_routing_step()`（首次报工）和 `ProductionReceipt::confirm()`（完工入库）事务内，追加直接 repo 级条件 UPDATE 调用，将状态变更传播到上游工单和计划。新增 `WorkOrderStatus::InProduction` 中间态。

**Tech Stack:** Rust (edition 2024), sqlx (raw SQL), Axum + Maud + HTMX, PostgreSQL

**Design Spec:** `docs/superpowers/specs/2026-06-13-mes-three-layer-linkage-design.md`


> **⚠️ 评审修订（2026-06-13）**：本计划已经过 feature-review 六角色评审。多处 Task 有重要修订——
> - Task 1 必须同步更新 3 处 `wo_status_label()` match 臂 + Dashboard SQL（否则编译失败）
> - Task 3 修复了 `--` 语法错误、枚举类型混淆、缺失状态守卫
| 文件 | 职责 | 变更类型 |
|------|------|---------|
| `abt-core/src/mes/enums.rs` | MES 枚举定义 | 修改：新增 InProduction |
| `abt-core/src/mes/dashboard/repo.rs` | Dashboard 统计 | 修改：`status IN (2,3)` → `IN (2,3,6)`（评审 P0） |
| `abt-core/src/mes/work_order/repo.rs` | 工单 DB 操作 | 修改：新增 update_status_conditional |
| `abt-core/src/mes/work_order/service.rs` | 工单 Service trait | 修改：新增 mark_in_production |
| `abt-core/src/mes/work_order/implt.rs` | 工单 Service 实现 | 修改：传播 + 状态条件放宽 + unrelease 回退扩展 |
| `abt-core/src/mes/production_plan/repo.rs` | 计划 DB 操作 | 修改：新增 3 个方法（含状态守卫修正） |
| `abt-core/src/mes/production_batch/implt.rs` | 批次 Service 实现 | 修改：confirm_routing_step 传播 |
| `abt-core/src/mes/production_receipt/implt.rs` | 完工入库 Service 实现 | 修改：confirm 传播（含多批次守卫） |
| `abt-web/src/pages/mes_report_create.rs` | 报工 handler | 修改：包显式事务（评审 P0） |
| `abt-web/src/pages/mes_receipt_detail.rs` | 入库确认 handler | 修改：包显式事务（评审 P0） |
| `abt-web/src/pages/mes_order_detail.rs` | 工单详情页 | 修改：wo_status_label + 取消按钮 + 来源追溯/批次状态 section |
| `abt-web/src/pages/mes_order_list.rs` | 工单列表页 | 修改：wo_status_label + parse_wo_status（评审 P0） |
| `abt-web/src/pages/mes_plan_detail.rs` | 计划详情页 | 修改：wo_status_label + tab_result 补 completed_steps |
| `abt-web/src/pages/mes_batch_detail.rs` | 批次详情页 | 修改：补设计划编号链接 |
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
- [ ] **Step 2: 同步更新 3 处 `wo_status_label()` exhaustive match（P0 编译失败）**

新增枚举变体会导致以下 3 个无 `_ =>` 通配臂的 match 编译失败。在 `abt-core` 和 `abt-web` 各加一臂：

1. `abt-web/src/pages/mes_order_detail.rs:27-33` — `wo_status_label()`:
```rust
InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
```

2. `abt-web/src/pages/mes_order_list.rs:32-40` — `wo_status_label()`:
```rust
InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
```

3. `abt-web/src/pages/mes_plan_detail.rs:57-63` — `wo_status_label()`:
```rust
WorkOrderStatus::InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
```

4. `abt-web/src/pages/mes_order_list.rs:42-43` — `parse_wo_status()`:
```rust
"InProduction" => Some(InProduction),
```

- [ ] **Step 3: 修复 Dashboard 硬编码状态列表（P0 数据失真）**

`abt-core/src/mes/dashboard/repo.rs:53` — `status IN (2,3)` 漏掉 InProduction(6)：
```sql
-- 改为：
(SELECT COUNT(*) FROM work_orders WHERE status IN (2,3,6)) AS active_orders
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/enums.rs abt-core/src/mes/dashboard/repo.rs \
  abt-web/src/pages/mes_order_detail.rs abt-web/src/pages/mes_order_list.rs \
  abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat(mes): add WorkOrderStatus::InProduction + update all match arms and dashboard"
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
    /// 前向守卫：仅当 PlanItem 当前状态为 Released(2) 或 InProduction(3) 时才更新，
    /// 防止将已终态（Cancelled=5）的 PlanItem 回退。
    pub async fn update_item_status_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
        status: PlanItemStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_plan_items ppi
            SET status = $2
            WHERE ppi.id = (
                SELECT wo.plan_item_id FROM work_orders wo
                WHERE wo.id = $1 AND wo.plan_item_id IS NOT NULL
            )
            AND ppi.status IN (2, 3)
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

    /// 重新计算计划状态：
    /// - 全部 PlanItem 为 Cancelled → Plan = Cancelled
    /// - 全部 PlanItem 为终态（Completed/Cancelled）且至少一个 Completed → Plan = Completed
    /// 条件 UPDATE，幂等（WHERE status = InProgress 保证只推进一次）。
    pub async fn recalculate_plan_status(
        executor: &mut sqlx::postgres::PgConnection,
        plan_id: i64,
    ) -> Result<()> {
        // 分支 A：全部 Cancelled → Plan = Cancelled
        sqlx::query(
            r#"
            UPDATE production_plans
            SET status = $2, updated_at = NOW()
            WHERE id = $1
              AND status = $3
              AND NOT EXISTS (
                SELECT 1 FROM production_plan_items
                WHERE plan_id = $1 AND status != $4
              )
            "#,
        )
        .bind(plan_id)                           // $1
        .bind(PlanStatus::Cancelled)             // $2: Plan 目标状态
        .bind(PlanStatus::InProgress)            // $3: Plan 当前状态条件
        .bind(PlanItemStatus::Cancelled)         // $4: PlanItemStatus
        .execute(&mut *executor)
        .await?;

        // 分支 B：全部终态且至少一个 Completed → Plan = Completed
        sqlx::query(
            r#"
            UPDATE production_plans
            SET status = $2, updated_at = NOW()
            WHERE id = $1
              AND status = $3
              AND NOT EXISTS (
                SELECT 1 FROM production_plan_items
                WHERE plan_id = $1
                  AND status NOT IN ($4, $5)
              )
              AND EXISTS (
                SELECT 1 FROM production_plan_items
                WHERE plan_id = $1 AND status = $4
              )
            "#,
        )
        .bind(plan_id)                           // $1
        .bind(PlanStatus::Completed)             // $2: Plan 目标状态
        .bind(PlanStatus::InProgress)            // $3: Plan 当前状态条件
        .bind(PlanItemStatus::Completed)         // $4: PlanItemStatus 终态
        .bind(PlanItemStatus::Cancelled)         // $5: PlanItemStatus 终态
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
```

**评审修正说明**：
- `update_item_status_by_work_order` 增加了 `AND ppi.status IN (2, 3)` 前向守卫（原版无此条件，会将 Cancelled 回退为 Completed）
- `recalculate_plan_status` 修正了 `--` 注释语法错误（Rust 中 `--` 不是注释）和枚举类型混淆（原版用 `PlanStatus::Completed` 绑定到 PlanItemStatus 列）
- 新增"全 Cancelled → Plan Cancelled"分支
```

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
- Modify: `abt-web/src/pages/mes_report_create.rs`（事务包裹）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（传播逻辑）

- [ ] **Step 0（P0 前置）：web handler 包显式事务**

当前 `mes_report_create.rs:101` 传 `&mut conn`（裸连接 autocommit），传播失败无法回滚报工。
必须改为显式事务：

```rust
// mes_report_create.rs — 原:
// svc.confirm_routing_step(&service_ctx, &mut conn, form.batch_id, form.step_no, req).await?;

// 改为:
let mut tx = state.pool().begin().await?;
svc.confirm_routing_step(&service_ctx, &mut *tx, form.batch_id, form.step_no, req).await?;
tx.commit().await?;
```

确认 `AppState` 是否暴露 `pool()` 方法；如无，新增 `pub fn pool(&self) -> &PgPool`。

- [ ] **Step 1: 在 confirm_routing_step 末尾追加传播逻辑**

在 `confirm_routing_step` 方法中，步骤 l（返回 StepConfirmationResult）之前，新增传播逻辑。
找到方法末尾的 `// --- l. 返回结果 ---` 注释行，在其 **之前** 插入：

```rust
        // --- k2. 状态传播：首次报工时推进上游工单和计划行状态 ---
        // batch.status 是步骤 a 读取的原始值（Pending 表示首次报工）
        if was_inserted && batch.status == BatchStatus::Pending {
            // WorkOrder: Released → InProduction
            new_work_order_service(self.pool.clone())
                .mark_in_production(ctx, db, batch.work_order_id)
                .await?;

            // PlanItem: Released → InProduction
            crate::mes::production_plan::repo::ProductionPlanRepo::update_item_status_by_work_order(
                &mut *db,
                batch.work_order_id,
                PlanItemStatus::InProduction,
            )
            .await?;
        }
```

**评审修正（P0）**：
- **传播错误必须用 `?` 而非 `tracing::warn!`**。报工 handler 已包事务（Step 0），传播失败会回滚整个事务——车间工人看到错误后重试即可。非阻塞 `warn!` 会导致三层状态静默漂移。
- `batch.status` 是步骤 a 读取的原始值，不会被步骤 k 修改（变量从未重新赋值）。用原始 `Pending` 值判断首次报工是正确的。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_report_create.rs abt-core/src/mes/production_batch/implt.rs
git commit -m "feat(mes): propagate batch in-progress to work order and plan item on first report"
```
---

### Task 6: ProductionReceipt::confirm 追加完工传播

**Files:**
- Modify: `abt-web/src/pages/mes_receipt_detail.rs`（事务包裹）
- Modify: `abt-core/src/mes/production_receipt/implt.rs`（传播逻辑）

- [ ] **Step 0（P0 前置）：web handler 包显式事务**

当前 `mes_receipt_detail.rs:71` 传 `&mut conn`（裸连接 autocommit）。改为显式事务：

```rust
// mes_receipt_detail.rs confirm_receipt() — 原:
// state.production_receipt_service().confirm(&service_ctx, &mut conn, path.receipt_id).await?;

// 改为:
let mut tx = state.pool().begin().await?;
state.production_receipt_service().confirm(&service_ctx, &mut *tx, path.receipt_id).await?;
tx.commit().await?;
```

- [ ] **Step 1: 在 confirm 方法步骤 6 之后追加传播**

找到 confirm 方法中步骤 6（`Update batch status to Completed`）之后的 `Ok(())` 之前，插入传播逻辑。
当前文件已 `use super::super::enums::*`，`WorkOrderStatus` 和 `PlanItemStatus` 已在作用域内。

```rust
        // --- 7. 状态传播：完工入库后推进上游工单和计划行状态 ---

        // 7a. 多批次守卫：检查该 WO 下是否所有批次都已终态
        let all_batches = ProductionBatchRepo::list_by_work_order(
            &mut *db, receipt.work_order_id,
        ).await.map_err(|e| DomainError::Internal(e.into()))?;
        let has_active_batch = all_batches.iter().any(|b| {
            b.status != BatchStatus::Completed && b.status != BatchStatus::Cancelled
        });

        // 7b. WorkOrder: InProduction → Closed（仅当所有批次终态时）
        if !has_active_batch {
            match WorkOrderRepo::update_status_conditional(
                &mut *db,
                receipt.work_order_id,
                WorkOrderStatus::InProduction,
                WorkOrderStatus::Closed,
            ).await {
                Ok(true) => {
                    // 仅在实际更新时记审计
                    new_audit_log_service(self.pool.clone())
                        .record(ctx, db,
                            RecordAuditLogReq::new("WorkOrder", receipt.work_order_id, AuditAction::Transition),
                        ).await?;
                }
                Ok(false) => {} // 状态不匹配（可能已 Closed），跳过
                Err(e) => return Err(DomainError::Internal(e.into())),
            }
        }

        // 7c. PlanItem: InProduction → Completed
        ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            receipt.work_order_id,
            PlanItemStatus::Completed,
        ).await?;

        // 7d. Plan: 重新计算状态
        if let Some(plan_id) = ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db, receipt.work_order_id,
        ).await? {
            ProductionPlanRepo::recalculate_plan_status(&mut *db, plan_id).await?;
        }
```

**评审修正（P0/P1）**：
- **多批次守卫**（P1）：新增 7a 检查 `list_by_work_order` 所有批次终态。有活跃批次则不关闭 WO，避免 split_work_order 多批次场景下提前关闭。
- **审计日志修正**（P1）：原版 `else` 分支在 `Ok(false)` 时也触发审计。改为 `Ok(true)` 才记录。
- **传播用 `?`**（P0）：原版 `tracing::warn!` 吞错误导致状态漂移。handler 已包事务，`?` 传播可回滚。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_receipt_detail.rs abt-core/src/mes/production_receipt/implt.rs
git commit -m "feat(mes): propagate receipt confirm to work order close and plan item completed"
```

---

### Task 7: WorkOrder::cancel 追加 PlanItem 传播

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: 在 cancel 方法中追加传播**

在 `cancel()` 方法中，审计日志之后、`Ok(())` 之前，追加（用 `?` 传播错误）：

```rust
        // 状态传播：PlanItem → Cancelled + 重新计算 Plan 状态
        crate::mes::production_plan::repo::ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            id,
            crate::mes::enums::PlanItemStatus::Cancelled,
        ).await?;

        if let Some(plan_id) = crate::mes::production_plan::repo::ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db, id,
        ).await? {
            crate::mes::production_plan::repo::ProductionPlanRepo::recalculate_plan_status(
                &mut *db, plan_id,
            ).await?;
        }
```

**评审修正（P0）**：原版 `tracing::warn!` 吞错误。改为 `?` 传播。cancel handler (`mes_order_detail.rs:255`) 传 `&mut conn`——建议同样包事务，但 cancel 操作的风险较低（WO 已标记 Cancelled + soft_delete），可作为 P1 后续处理。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs
git commit -m "feat(mes): propagate work order cancel to plan item and recalculate plan status"
```

---

### Task 8: 放宽 close() 和 cancel() 状态接受条件 + UI 按钮更新

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`
- Modify: `abt-web/src/pages/mes_order_detail.rs`
- Modify: `abt-core/src/mes/work_order/implt.rs`（unrelease 回退条件）

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

- [ ] **Step 3（评审新增 P1）：unrelease() PlanItem 回退条件扩展**

`implt.rs:414-419` 当前回退条件仅接受 `status = Released(2)`。如果传播成功后 PlanItem 已是 InProduction(3)，
回退不命中。虽然 unrelease 在 WO 非 Released 时被拦截（line 326），但传播失败时 WO 仍为 Released 而 PlanItem 可能已是 InProduction。
扩展回退条件同时接受 Released 和 InProduction：

```rust
// 原（line 414-419）:
// "UPDATE production_plan_items SET status = $2 WHERE id = $1 AND status = $3"
// .bind(PlanItemStatus::Released)  // $3

// 改为:
"UPDATE production_plan_items SET status = $2 WHERE id = $1 AND status IN ($3, $4)"
.bind(plan_item_id)
.bind(PlanItemStatus::Planned)       // $2
.bind(PlanItemStatus::Released)      // $3
.bind(PlanItemStatus::InProduction)  // $4
```

- [ ] **Step 4（评审新增 P0）：UI 取消按钮增加 InProduction**

`mes_order_detail.rs:343` — cancel 按钮条件需增加 InProduction：
```rust
// 原:
@if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released) {
// 改为:
@if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned
            | WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
```

- [ ] **Step 5: 验证编译**

Run: `cargo clippy 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 6: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs abt-web/src/pages/mes_order_detail.rs
git commit -m "fix(mes): close/cancel/unrelease accept InProduction + UI cancel button"
```
## Phase 3: 详情页关联信息

> **评审结论**：经代码核实，Task 9/10/11 的工作量被高估约 70%——Tab、链接、关联数据**均已存在**。
> 以下为精简后的实际改动。

### Task 9: 计划详情页 — 补充 completed_steps 显示

**评审发现**："下达结果" Tab **已存在**——`mes_plan_detail.rs:337` 已注册 Tab、`:342` 已渲染面板、`:481-527` 已有 `tab_result()` 函数、`:206` 已加载 `work_orders`。Tab 切换使用现有 `detail_tabs()` + `tab_panel()` 机制。**禁止引入 Surreal.js `me().on('click')`**。

**唯一改动**：`tab_result()` 的 `:511-512` 当前仅显示 `total_steps`，补充 `completed_steps`：

**Files:**
- Modify: `abt-web/src/pages/mes_plan_detail.rs:511-512`

- [ ] **Step 1: 补充 completed_steps 显示**

找到 `tab_result()` 函数中的工序显示（约 line 511-512），将：
```rust
@if let Some(steps) = wo.total_steps {
    span { "工序: " (steps) "步" }
}
```
改为：
```rust
@if let (Some(done), Some(total)) = (wo.completed_steps, wo.total_steps) {
    span { "工序: " (done) "/" (total) "步" }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat(mes-ui): plan detail tab_result - show completed/total steps"
```

---

### Task 10: 工单详情页 — 渲染来源追溯 + 批次执行状态

**评审发现**：数据**全部已加载**——`order.source_plan_doc`、`order.source_so_doc`、`order.source_customer` 已由 `WorkOrderRepo::get_by_id` SQL JOIN 填充（repo.rs:64-65）。`batches`（handler line 123）和 `routings`（handler line 117）也已在 handler 中加载。**无需任何新增查询，禁止 abt-web 直接 SQL。**

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs`

- [ ] **Step 1: 在 `order_detail_page` 中新增"来源追溯" section**

在 `tab_info()` 函数（:503）中或单独新增一个 section，渲染已有的 `order` 字段：

```rust
section class="sub-section" {
    div class="sub-section-title" { "来源追溯" }
    div class="detail-info-grid" {
        @if let (Some(pid), Some(pdoc)) = (order.source_plan_id, &order.source_plan_doc) {
            div class="detail-info-item" {
                span class="detail-info-label" { "计划编号" }
                span class="detail-info-value" {
                    a href=(format!("/admin/mes/plans/{}", pid)) class="link-cell" { (pdoc) }
                }
            }
        }
        @if let Some(so_doc) = &order.source_so_doc {
            div class="detail-info-item" {
                span class="detail-info-label" { "销售订单" }
                span class="detail-info-value" { (so_doc) }
            }
        }
        @if let Some(customer) = &order.source_customer {
            div class="detail-info-item" {
                span class="detail-info-label" { "客户" }
                span class="detail-info-value" { (customer) }
            }
        }
    }
}
```

**注意**：`order_detail_page` 函数签名已接收 `order: &WorkOrder`（:283），`batches: &[ProductionBatch]`（:286），`routings: &[WorkOrderRouting]`（:285）——所有数据已传入，无需修改 handler。

- [ ] **Step 2: 新增"批次执行状态" section**

在同一页面新增批次概览 section，遍历已传入的 `batches`：

```rust
section class="sub-section" {
    div class="sub-section-title" { "批次执行状态" }
    @if batches.is_empty() {
        div style="color:var(--muted)" { "暂无批次" }
    } @else {
        div class="data-card-scroll" {
            table class="data-table" {
                thead { tr { th {"批次"} th {"流转卡"} th {"状态"} th {"当前工序"} th {"完成/报废"} } }
                tbody {
                    @for b in batches {
                        tr {
                            td { a href=(format!("/admin/mes/batches/{}", b.id)) class="link-cell mono" { (b.batch_no) } }
                            td class="mono" { (b.card_sn) }
                            td { (batch_status_pill(b.status)) }
                            td { (b.current_step) }
                            td class="mono" { (crate::utils::fmt_qty(b.completed_qty)) " / " (crate::utils::fmt_qty(b.scrap_qty)) }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat(mes-ui): work order detail - add source tracing and batch status sections"
```

---

### Task 11: 批次详情页 — 补充计划编号链接

**评审发现**：工单编号链接**已存在**——`mes_batch_detail.rs:196` 已有 `a href="/admin/mes/orders/{wo.id}"`。`wo` 已在 handler line 40 通过 `find_by_id` 加载，`source_plan_doc`/`source_plan_id` 已由 `get_by_id` SQL 填充。**唯一缺失：计划编号链接。**

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs:196` 之后

- [ ] **Step 1: 在工单 info-item 之后新增计划 info-item**

在 `:196`（工单链接行）之后插入：

```rust
@if let (Some(pid), Some(pdoc)) = (wo.source_plan_id, &wo.source_plan_doc) {
    div class="detail-info-item" {
        span class="detail-info-label" { "计划" }
        span class="detail-info-value" {
            a href=(format!("/admin/mes/plans/{}", pid)) class="link-cell" { (pdoc) }
        }
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | findstr "error"`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_batch_detail.rs
git commit -m "feat(mes-ui): batch detail - add plan navigation link"
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

### 原始检查项
- [x] **Spec coverage**: 设计文档中的每个传播点（首次报工、完工入库、取消、反下达）都有对应 Task
- [x] **Placeholder scan**: 无 TBD/TODO，每个 step 有具体代码
- [x] **Type consistency**: `WorkOrderStatus::InProduction = 6`、`PlanItemStatus::InProduction = 3`（已有枚举值，无需新增）、`update_status_conditional` 签名一致
- [x] **close()/cancel() 接受 InProduction** — Task 8 覆盖

### 评审修订检查项（2026-06-13 feature-review）
- [x] **P0 枚举爆破**：3 处 `wo_status_label()` match 臂 + `parse_wo_status` + Dashboard SQL — Task 1 Step 2/3 覆盖
- [x] **P0 语法错误**：`recalculate_plan_status` 的 `--` 注释 → `//` — Task 3 修复
- [x] **P0 枚举类型混淆**：PlanStatus / PlanItemStatus 独立 bind 参数 — Task 3 修复
- [x] **P0 缺状态守卫**：`update_item_status_by_work_order` 增加 `AND ppi.status IN (2,3)` — Task 3 修复
- [x] **P0 事务策略**：web handler 包显式事务，传播用 `?` — Task 5 Step 0 / Task 6 Step 0 修复
- [x] **P0 Surreal.js 违规**：Task 9 已删除 Surreal.js 代码，改为现有 Tab 机制
- [x] **P0 abt-web SQL 违规**：Task 10 已删除 `sqlx::query_as`，改用已有 `order` 字段
- [x] **P1 多批次守卫**：Task 6 增加 7a 批次终态检查
- [x] **P1 审计误触发**：Task 6 改为 `Ok(true)` 才记审计
- [x] **P1 unrelease 回退**：Task 8 Step 3 扩展 PlanItem 回退条件
- [x] **P1 UI 取消按钮**：Task 8 Step 4 增加 InProduction
- [x] **P1 全 Cancelled → Plan Cancelled**：Task 3 `recalculate_plan_status` 分支 A
- [x] **Task 9/10/11 精简**：Tab/链接/数据均已存在，工作量缩减约 70%
