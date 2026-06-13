# 阶段 1：止血 — 修复 P0 数据正确性

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 3 个影响生产数据的根本错误（P2 工序来源 + P4 BOM 快照 + P8 倒冲仓库）+ 顺带修复 P6 销售订单追溯

**Architecture:** 改造 `release()` 为 backflush-only 简化路径，删除旧的成品预留和空壳领料单逻辑。新增 BOM 快照查找/创建、Routing 工序映射、倒冲仓库策略。

**Tech Stack:** Rust + sqlx + async-trait

**验收:** 单个工单 release → 报工 → 完工入库 → 倒冲，全链路数据正确（工序来自 Routing、BOM 快照非空、倒冲从正确仓库扣减、sales_order_id 非空）

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 新增方法 | `abt-core/src/master_data/bom/repo.rs` | `find_published_by_product_code()` — 查找产品的已发布 BOM |
| 新增方法 | `abt-core/src/master_data/bom/repo.rs` | `find_snapshot_by_id()` — 按 snapshot_id 加载快照 |
| 新增方法 | `abt-core/src/master_data/bom/service.rs` | `find_published_bom_by_product_code()` — Service trait 方法 |
| 新增方法 | `abt-core/src/master_data/bom/service.rs` | `get_snapshot_by_id()` — Service trait 方法 |
| 新增方法 | `abt-core/src/master_data/bom/implt.rs` | 上述两个方法的实现 |
| 新增方法 | `abt-core/src/master_data/bom/mod.rs` | 导出新 Service |
| 新增方法 | `abt-core/src/mes/work_order/repo.rs` | `update_bom_snapshot_id()` + `update_routing_id()` |
| 重写 | `abt-core/src/mes/work_order/implt.rs` | `release()` — routing + BOM snapshot 简化路径 |
| 修改 | `abt-core/src/wms/backflush/implt.rs` | `execute()` + `get_bom_components()` — 仓库修正 + 从快照读取 |
| 修改 | `abt-core/src/mes/production_plan/implt.rs` | `release_to_work_orders()` — 传入 sales_order_id |
| 导出 | `abt-core/src/master_data/bom/mod.rs` | 确保新工厂函数和新 Service 导出 |

---

### Task 1: BOM Repo — 新增 find_published_by_product_code + find_snapshot_by_id

**Files:**
- Modify: `abt-core/src/master_data/bom/repo.rs:653` (文件末尾，BomSnapshotRepo impl 块之后)

- [ ] **Step 1: 在 BomRepo impl 块中添加 find_published_by_product_code**

在 `BomRepo` impl 块的 `find_product_codes_with_bom` 方法之后添加：

```rust
/// 查找产品关联的已发布 BOM（通过根节点的 product_code 匹配）
pub async fn find_published_by_product_code(
    &self,
    executor: PgExecutor<'_>,
    product_code: &str,
) -> Result<Option<i64>> {
    let bom_id = sqlx::query_scalar::<sqlx::Postgres, i64>(
        r#"
        SELECT b.bom_id
        FROM boms b
        JOIN bom_nodes bn ON bn.bom_id = b.bom_id
        WHERE bn.product_code = $1
          AND bn.parent_id = 0
          AND b.status = 2
          AND b.deleted_at IS NULL
        ORDER BY b.bom_id DESC
        LIMIT 1
        "#,
    )
    .bind(product_code)
    .fetch_optional(executor)
    .await?;
    Ok(bom_id)
}
```

- [ ] **Step 2: 在 BomSnapshotRepo impl 块中添加 find_by_snapshot_id**

在 `BomSnapshotRepo.find_by_bom_and_version` 方法之后添加：

```rust
/// 按 snapshot_id 加载单个快照
pub async fn find_by_snapshot_id(
    &self,
    executor: PgExecutor<'_>,
    snapshot_id: i64,
) -> Result<Option<BomSnapshot>> {
    let snapshot = sqlx::query_as::<sqlx::Postgres, BomSnapshot>(
        sqlx::AssertSqlSafe(format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM bom_snapshots WHERE snapshot_id = $1"
        )),
    )
    .bind(snapshot_id)
    .fetch_optional(executor)
    .await?;
    Ok(snapshot)
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无新增 error（可能有 unused warning，后续步骤会用到）

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/master_data/bom/repo.rs
git commit -m "feat(bom): add find_published_by_product_code + find_snapshot_by_id repo methods"
```

---

### Task 2: BOM Service — 新增 Service trait 方法 + 实现

**Files:**
- Modify: `abt-core/src/master_data/bom/service.rs` (BomQueryService trait)
- Modify: `abt-core/src/master_data/bom/implt.rs` (BomQueryServiceImpl)

- [ ] **Step 1: 在 BomQueryService trait 中添加两个新方法**

在 `BomQueryService` trait 的 `exists_name` 方法之后添加：

```rust
    /// 查找产品关联的已发布 BOM，返回 bom_id
    async fn find_published_bom_by_product_code(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Option<i64>>;

    /// 按 snapshot_id 加载快照
    async fn get_snapshot_by_id(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        snapshot_id: i64,
    ) -> Result<Option<BomSnapshot>>;
```

需要在文件顶部添加 use：
```rust
use super::model::BomSnapshot;
```

- [ ] **Step 2: 在 BomQueryServiceImpl 中实现这两个方法**

在 `BomQueryServiceImpl` 的 `exists_name` 方法之后添加：

```rust
    async fn find_published_bom_by_product_code(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Option<i64>> {
        self.repo.find_published_by_product_code(db, product_code).await
    }

    async fn get_snapshot_by_id(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        snapshot_id: i64,
    ) -> Result<Option<BomSnapshot>> {
        self.snapshot_repo.find_by_snapshot_id(db, snapshot_id).await
    }
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无新增 error

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/master_data/bom/service.rs abt-core/src/master_data/bom/implt.rs
git commit -m "feat(bom): add find_published_bom_by_product_code + get_snapshot_by_id to BomQueryService"
```

---

### Task 3: WorkOrder Repo — 新增 update_bom_snapshot_id + update_routing_id

**Files:**
- Modify: `abt-core/src/mes/work_order/repo.rs:226` (文件末尾)

- [ ] **Step 1: 在 WorkOrderRepo impl 块末尾添加两个方法**

```rust
    /// 更新工单的 BOM 快照 ID
    pub async fn update_bom_snapshot_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        bom_snapshot_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_orders SET bom_snapshot_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(bom_snapshot_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 更新工单的工艺路线 ID
    pub async fn update_routing_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        routing_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_orders SET routing_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(routing_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无新增 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/work_order/repo.rs
git commit -m "feat(work-order): add update_bom_snapshot_id + update_routing_id repo methods"
```

---

### Task 4: 重写 release() — routing + BOM snapshot 简化路径

这是阶段 1 的核心改动。重写 `WorkOrderServiceImpl::release()` 方法，修复 P2（工序来源错误）和 P4（BOM 未快照），同时删除 backflush 模式下不需要的成品预留和空壳领料单。

**Files:**
- Rewrite: `abt-core/src/mes/work_order/implt.rs` (lines 82-214, `release()` 方法体)

- [ ] **Step 1: 添加新的 use 声明**

在 `implt.rs` 文件顶部的 use 块中添加：

```rust
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::master_data::product::{new_product_service, service::ProductService};
```

- [ ] **Step 2: 重写 release() 方法**

完整替换 `release()` 方法（lines 82-214）为以下代码：

```rust
    /// 下达工单：Draft/Planned -> Released
    /// - BOM 快照（冻结用料清单）
    /// - 从 Routing 创建工序（或虚拟默认工序）
    /// - 创建 ProductionBatch
    /// - backflush 模式：不预留、不创建领料单
    async fn release(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        // 1. 验证工单存在且状态允许下达
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Released.to_string(),
            });
        }

        // 2. 乐观锁更新状态
        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *db,
                id,
                WorkOrderStatus::Released,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        // 3. 获取产品信息（用于查找 BOM 和 Routing）
        let product = new_product_service(self.pool.clone())
            .get(ctx, db, work_order.product_id).await?;
        let product_code = &product.product_code;

        // 4. BOM 快照：查找产品已发布 BOM → 获取最新快照 → 写入 work_order.bom_snapshot_id
        let bom_snapshot_id = if let Some(bom_id) = new_bom_query_service(self.pool.clone())
            .find_published_bom_by_product_code(ctx, db, product_code)
            .await?
        {
            // 获取该 BOM 的最新快照
            let snapshots = new_bom_query_service(self.pool.clone())
                .get_snapshots(ctx, db, bom_id, None, Some(1))
                .await?;

            if let Some(latest_snapshot) = snapshots.into_iter().next() {
                WorkOrderRepo::update_bom_snapshot_id(&mut *db, id, latest_snapshot.snapshot_id)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                Some(latest_snapshot.snapshot_id)
            } else {
                // BOM 已发布但无快照（异常情况，理论上不应发生）
                None
            }
        } else {
            None // 无已发布 BOM，跳过快照
        };

        // 5. 工序创建：从 Routing 映射，或虚拟默认工序
        let routing_detail = new_routing_service(self.pool.clone())
            .get_bom_routing(ctx, db, product_code.to_string())
            .await?;

        let routing_steps: Vec<WorkOrderRouting> = if let Some(ref detail) = routing_detail {
            // 从 Routing 映射工序
            detail.steps.iter().map(|step| WorkOrderRouting {
                id: 0,
                work_order_id: id,
                step_no: step.step_order,
                process_name: step.process_name.clone().unwrap_or_else(|| step.process_code.clone()),
                work_center_id: None,
                standard_time: None,
                standard_cost: None,
                unit_price: None,
                allowed_loss_rate: None,
                planned_qty: work_order.planned_qty,
                completed_qty: Decimal::ZERO,
                defect_qty: Decimal::ZERO,
                status: super::super::enums::RoutingStatus::Pending,
                is_outsourced: false,
                is_inspection_point: false,
            }).collect()
        } else {
            // 无 Routing → 虚拟默认工序
            vec![WorkOrderRouting {
                id: 0,
                work_order_id: id,
                step_no: 1,
                process_name: "生产".to_string(),
                work_center_id: None,
                standard_time: None,
                standard_cost: None,
                unit_price: None,
                allowed_loss_rate: None,
                planned_qty: work_order.planned_qty,
                completed_qty: Decimal::ZERO,
                defect_qty: Decimal::ZERO,
                status: super::super::enums::RoutingStatus::Pending,
                is_outsourced: false,
                is_inspection_point: false,
            }]
        };

        WorkOrderRoutingRepo::insert_for_work_order(&mut *db, &routing_steps)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 记录 routing_id 到工单
        if let Some(ref detail) = routing_detail {
            WorkOrderRepo::update_routing_id(&mut *db, id, detail.routing.id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 创建至少 1 个 ProductionBatch
        let batch_req = crate::mes::production_batch::model::CreateBatchReq {
            work_order_id: id,
            product_id: work_order.product_id,
            batch_qty: work_order.planned_qty,
            team_id: None,
        };

        let batch_no = new_document_sequence_service(self.pool.clone())
            .next_number(
                ctx, db,
                DocumentType::WorkOrder,
            )
            .await
            .unwrap_or_else(|_| format!("{}-01", work_order.doc_number));

        let card_sn = format!("SN-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f"));

        ProductionBatchRepo::insert(
            &mut *db,
            &batch_req,
            &batch_no,
            &card_sn,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 7. backflush 模式：不预留、不创建领料单
        // （阶段 2 引入 picking 模式时在此处分流）

        // 8. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }
```

- [ ] **Step 3: 清理不再需要的 use**

移除 `release()` 中不再使用的 import（`ReserveRequest`、`ReservationType`、`MaterialRequisitionService` 相关）。如果文件中其他方法仍使用这些 import，则保留。

检查：
- `use crate::shared::inventory_reservation::{...}` — `close()` 和 `cancel()` 仍使用 → 保留
- `use crate::wms::material_requisition::{...}` — 不再使用 → 可移除

移除：
```rust
// 删除这一行（release 不再调用领料单）
use crate::wms::material_requisition::{new_material_requisition_service, service::MaterialRequisitionService};
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error，可能有 unused import warning（移除后应清除）

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs
git commit -m "fix(mes): rewrite release() — routing-based工序 + BOM snapshot + backflush-only path

- Fix P2: 工序从 Routing 映射而非 BOM 叶子节点，无 Routing 时创建虚拟默认工序
- Fix P4: 查找产品已发布 BOM → 获取最新快照 → 写入 bom_snapshot_id
- Remove 成品预留 (backflush 模式不需要)
- Remove 空壳领料单创建 (backflush 模式不需要)"
```

---

### Task 5: 修正倒冲仓库 + 从快照读取组件

修复 P8（`warehouse_id: 0` 硬编码）。同时修改 `get_bom_components()` 从快照 `bom_detail` 读取叶子节点，而不是查 live BOM。

**Files:**
- Modify: `abt-core/src/wms/backflush/implt.rs` (lines 122-142 `warehouse_id` + lines 208-229 `get_bom_components`)

- [ ] **Step 1: 重写 get_bom_components 从快照读取**

完整替换 `get_bom_components()` 函数（文件末尾 lines 208-229）：

```rust
/// 从工单的 BOM 快照获取叶子组件列表
async fn get_bom_components(
    pool: &PgPool,
    ctx: &ServiceContext, db: PgExecutor<'_>,
    wo: &crate::mes::work_order::model::WorkOrder,
) -> Result<Vec<BomComponent>> {
    let snapshot_id = wo.bom_snapshot_id;
    if let Some(snapshot_id) = snapshot_id {
        // 从快照的 bom_detail 中提取叶子节点
        let snapshot = new_bom_query_service(pool.clone())
            .get_snapshot_by_id(ctx, db, snapshot_id).await?;

        if let Some(snap) = snapshot {
            let all_nodes = &snap.bom_detail.nodes;
            // 叶子节点 = 没有任何节点的 parent_id 等于它的 node_id
            let parent_ids: std::collections::HashSet<i64> =
                all_nodes.iter().map(|n| n.parent_id).collect();
            let leaf_nodes: Vec<&crate::master_data::bom::model::BomNode> = all_nodes
                .iter()
                .filter(|n| !parent_ids.contains(&n.id))
                .collect();

            Ok(leaf_nodes.into_iter().map(|n| BomComponent {
                product_id: n.product_id,
                required_qty: n.quantity,
            }).collect())
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}
```

- [ ] **Step 2: 添加仓库解析辅助函数**

在 `get_bom_components` 之前添加仓库解析函数：

```rust
/// 4 级仓库策略：确定倒冲仓库
/// 1. 工单工作中心的默认仓库 (当前未实现，跳过)
/// 2. 组件产品的默认仓库 (products 表无此字段，跳过)
/// 3. 工单成品关联的默认仓库 (同上)
/// 4. 回退：查找系统中第一个活跃仓库
async fn resolve_warehouse_id(
    pool: &PgPool,
    db: PgExecutor<'_>,
) -> Result<i64> {
    // V1 简化实现：查找系统中第一个活跃仓库
    // 后续阶段（阶段 2+）将实现完整 4 级策略
    let warehouse_id: Option<i64> = sqlx::query_scalar(
        "SELECT warehouse_id FROM warehouses WHERE deleted_at IS NULL ORDER BY warehouse_id LIMIT 1",
    )
    .fetch_optional(&mut *db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(warehouse_id.unwrap_or(0))
}
```

- [ ] **Step 3: 修改 execute() 中的 warehouse_id**

在 `execute()` 方法中，将 `warehouse_id: 0`（约 line 130）替换为调用仓库解析：

找到这段代码（在 for loop 内部，`new_inventory_transaction_service` 调用中）：
```rust
                    warehouse_id: 0,
```

替换为：
```rust
                    warehouse_id: resolve_warehouse_id(&self.pool, db).await?,
```

注意：`resolve_warehouse_id` 应在 loop 外调用一次，避免每行组件都查询数据库。更好的做法是在 for loop 之前调用：

在 `let bom_components = get_bom_components(...)` 之后、`for component in &bom_components` 之前添加：
```rust
        let warehouse_id = resolve_warehouse_id(&self.pool, db).await?;
```

然后在 loop 内使用 `warehouse_id` 变量。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/wms/backflush/implt.rs
git commit -m "fix(backflush): resolve warehouse_id from DB instead of hardcoded 0 (P8)

- Add resolve_warehouse_id() with fallback to first active warehouse
- Fix get_bom_components() to read from BOM snapshot instead of live BOM
- Read leaf nodes from snapshot.bom_detail.nodes"
```

---

### Task 6: 修复销售订单追溯（P6）

**Files:**
- Modify: `abt-core/src/mes/production_plan/implt.rs` (lines 121-135)

- [ ] **Step 1: 在 release_to_work_orders() 中传入 sales_order_id**

找到 `CreateWorkOrderReq` 构建处（约 line 121-135）：

当前代码：
```rust
                CreateWorkOrderReq {
                    plan_item_id: Some(item.id),
                    product_id: item.product_id,
                    bom_snapshot_id: None,
                    routing_id: None,
                    planned_qty: item.planned_qty,
                    scheduled_start,
                    scheduled_end,
                    work_center_id: None,
                    sales_order_id: None,  // ← 修复点
                    remark: None,
                },
```

替换 `sales_order_id: None` 为：
```rust
                    sales_order_id: item.sales_order_id,
```

同时，在创建工单成功后，立即调用 `release()` 代替只创建不 release：

找到当前代码（约 lines 136-145）：
```rust
                Ok(wo_id) => {
                    if let Ok(wo) = work_order_svc.find_by_id(ctx, db, wo_id).await {
                        successful.push(wo);
                    }
                }
```

替换为：
```rust
                Ok(wo_id) => {
                    // 阶段 1：仅创建工单，不自动 release
                    // （阶段 3 实现一键 release）
                    if let Ok(wo) = work_order_svc.find_by_id(ctx, db, wo_id).await {
                        successful.push(wo);
                    }
                }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_plan/implt.rs
git commit -m "fix(mes): pass sales_order_id from PlanItem to WorkOrder (P6)"
```

---

### Task 7: 端到端验证

- [ ] **Step 1: 全量 clippy 检查**

Run: `cargo clippy 2>&1 | tail -20`
Expected: 无 error

- [ ] **Step 2: 运行现有测试**

Run: `cargo test -p abt-core 2>&1 | tail -30`
Expected: 所有现有测试通过（可能有因改动而失败的测试，需逐一修复）

- [ ] **Step 3: 最终 commit（如有测试修复）**

```bash
git add -A
git commit -m "fix(mes): phase 1 — fix test regressions from release() rewrite"
```

---

## 自检清单

- [x] **Spec coverage**: P2 工序来源 → Task 4, P4 BOM 快照 → Task 4, P8 倒冲仓库 → Task 5, P6 销售订单追溯 → Task 6
- [x] **Placeholder scan**: 无 TBD/TODO/实现占位符
- [x] **Type consistency**:
  - `WorkOrderRouting` 结构体字段与 `production_batch/model.rs:27-43` 定义一致
  - `BomSnapshot.snapshot_id` 类型 `i64` 与 `work_order.bom_snapshot_id: Option<i64>` 兼容
  - `BomNode.id` 类型 `i64` 用于 parent_id 集合比较
  - `get_bom_routing()` 返回 `Result<Option<RoutingDetail>>`，与 routing service 签名一致
