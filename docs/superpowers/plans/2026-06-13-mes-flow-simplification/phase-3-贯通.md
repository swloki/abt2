# 阶段 3：一键贯通 + 业务规则

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 需求池到工单一键贯通 + 物料可用性预检 + 超额生产容差控制 + 事件发布。

**Architecture:** 改造 `release_to_work_orders()` 为预校验 → 逐个创建+release → 失败隔离的模式。新增 `pre_validate()` 和 `ReleaseValidation` 模型。在 `confirm_routing_step()` 中增加超额容差校验。

**Tech Stack:** Rust + sqlx + async-trait

**前置:** 阶段 2 已完成

**验收:**
- 需求池选 3 条需求 → 一键下达 → 3 个 Released 工单
- 物料不足 → warning 显示但不下达阻断
- 部分失败 → 成功工单不受影响，失败行可重试
- 超额报工超 5% → 拒绝

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 修改 | `abt-core/src/mes/production_plan/model.rs` | 新增 `ReleaseValidation`、`MaterialShortage`、增强 `BatchReleaseResult` |
| 新增方法 | `abt-core/src/mes/production_plan/service.rs` | `pre_validate()` trait 方法 |
| 重写 | `abt-core/src/mes/production_plan/implt.rs` | `release_to_work_orders()` 一键到底 + `pre_validate()` |
| 修改 | `abt-core/src/mes/production_batch/implt.rs` | `confirm_routing_step()` 增加超额容差校验 |
| 修改 | `abt-core/src/master_data/product/model.rs` | `ProductMeta` 增加 `over_completion_tolerance` 字段 |
| 新增方法 | `abt-core/src/mes/production_plan/repo.rs` | `update_item_status()` |
| 修改 | `abt-core/src/mes/work_order/implt.rs` | release() + unrelease() 增加事件发布 |

---

### Task 1: 新增 ReleaseValidation + MaterialShortage 模型

**Files:**
- Modify: `abt-core/src/mes/production_plan/model.rs` (文件末尾)

- [ ] **Step 1: 在 model.rs 末尾添加新模型**

```rust
/// 下达预校验结果
#[derive(Debug, Clone)]
pub struct ReleaseValidation {
    pub plan_item_id: i64,
    pub product_id: i64,
    pub has_routing: bool,
    pub has_published_bom: bool,
    pub routing_id: Option<i64>,
    pub warnings: Vec<String>,
    pub material_shortages: Vec<MaterialShortage>,
}

/// 物料短缺信息
#[derive(Debug, Clone)]
pub struct MaterialShortage {
    pub product_id: i64,
    pub product_name: String,
    pub required_qty: rust_decimal::Decimal,
    pub available_qty: rust_decimal::Decimal,
    pub shortage_qty: rust_decimal::Decimal,
}
```

- [ ] **Step 2: 增强 BatchReleaseResult**

找到 `BatchReleaseResult` struct（约 line 72-77）：

```rust
pub struct BatchReleaseResult {
    pub plan_id: i64,
    pub successful_work_orders: Vec<WorkOrder>,
    pub failed_items: Vec<BatchFailure>,
    pub total: i32,
}
```

替换为：

```rust
pub struct BatchReleaseResult {
    pub plan_id: i64,
    pub successful_work_orders: Vec<WorkOrder>,
    pub failed_items: Vec<BatchFailure>,
    pub validations: Vec<ReleaseValidation>,
    pub total: i32,
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 编译错误（`release_to_work_orders` 中构建 BatchReleaseResult 缺少 validations 字段），下一任务修复

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/mes/production_plan/model.rs
git commit -m "feat(plan): add ReleaseValidation, MaterialShortage models + enhance BatchReleaseResult"
```

---

### Task 2: ProductionPlanService — 新增 pre_validate + 重写 release_to_work_orders

**Files:**
- Modify: `abt-core/src/mes/production_plan/service.rs`
- Rewrite: `abt-core/src/mes/production_plan/implt.rs`

- [ ] **Step 1: 在 ProductionPlanService trait 中添加 pre_validate**

```rust
    /// 预校验：检查 Routing、BOM、物料可用性
    async fn pre_validate(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<ReleaseValidation>>;
```

- [ ] **Step 2: 在 repo.rs 添加 update_item_status**

在 `ProductionPlanRepo` impl 中添加：

```rust
    /// 更新计划行状态
    pub async fn update_item_status(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        status: super::super::enums::PlanItemStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE production_plan_items SET status = $2 WHERE id = $1",
        )
        .bind(item_id)
        .bind(status)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
```

- [ ] **Step 3: 重写 release_to_work_orders + 实现 pre_validate**

完整替换 `ProductionPlanServiceImpl` 中的 `release_to_work_orders` 方法，并添加 `pre_validate` 实现。

需要在 implt.rs 顶部添加 use：

```rust
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::shared::inventory::{new_inventory_service, service::InventoryService};
```

注意：需要确认实际的 inventory service 路径和可用方法。如果不存在 `new_inventory_service`，需要用原始 SQL 查询库存。

重写 `release_to_work_orders`：

```rust
    async fn pre_validate(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<ReleaseValidation>> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let mut validations = Vec::new();

        for item in &items {
            let mut warnings = Vec::new();
            let mut material_shortages = Vec::new();

            // 检查产品
            let product = match new_product_service(self.pool.clone())
                .get(ctx, db, item.product_id).await
            {
                Ok(p) => p,
                Err(_) => {
                    warnings.push(format!("产品 ID {} 不存在", item.product_id));
                    validations.push(ReleaseValidation {
                        plan_item_id: item.id,
                        product_id: item.product_id,
                        has_routing: false,
                        has_published_bom: false,
                        routing_id: None,
                        warnings,
                        material_shortages,
                    });
                    continue;
                }
            };

            // 检查 Routing
            let routing_detail = new_routing_service(self.pool.clone())
                .get_bom_routing(ctx, db, product.product_code.clone())
                .await
                .ok()
                .flatten();

            let has_routing = routing_detail.is_some();
            if !has_routing {
                warnings.push("该产品无关联工艺路线，将使用虚拟默认工序".to_string());
            }

            // 检查已发布 BOM
            let bom_id = new_bom_query_service(self.pool.clone())
                .find_published_bom_by_product_code(ctx, db, &product.product_code)
                .await
                .ok()
                .flatten();

            let has_published_bom = bom_id.is_some();
            if !has_published_bom {
                warnings.push("该产品无已发布 BOM，将跳过快照和物料预检".to_string());
            }

            // 物料可用性预检（仅当有 BOM 时）
            if let Some(snap_id) = item.bom_snapshot_id.or_else(|| {
                // 如果 item 没有 snapshot，尝试获取 BOM 最新快照
                None
            }) {
                let snapshot_opt = new_bom_query_service(self.pool.clone())
                    .get_snapshot_by_id(ctx, db, snap_id).await
                    .ok()
                    .flatten();

                if let Some(snapshot) = snapshot_opt {
                    let all_nodes = &snapshot.bom_detail.nodes;
                    let parent_ids: std::collections::HashSet<i64> =
                        all_nodes.iter().map(|n| n.parent_id).collect();
                    let leaf_nodes: Vec<_> = all_nodes
                        .iter()
                        .filter(|n| !parent_ids.contains(&n.id))
                        .collect();

                    for node in &leaf_nodes {
                        let required_qty = node.quantity * item.planned_qty;
                        // 查询可用库存：on_hand - hard_reserved
                        let available: (rust_decimal::Decimal, rust_decimal::Decimal) =
                            sqlx::query_as(
                                r#"SELECT COALESCE(SUM(quantity), 0) as on_hand,
                                          COALESCE(SUM(CASE WHEN status = 'available' THEN quantity ELSE 0 END), 0) as available
                                   FROM inventories
                                   WHERE product_id = $1 AND deleted_at IS NULL"#,
                            )
                            .bind(node.product_id)
                            .fetch_one(&mut *db)
                            .await
                            .map_err(|e| DomainError::Internal(e.into()))?;

                        let available_qty = available.1;
                        if available_qty < required_qty {
                            let product_name = node.product_code.clone().unwrap_or_default();
                            material_shortages.push(MaterialShortage {
                                product_id: node.product_id,
                                product_name,
                                required_qty,
                                available_qty,
                                shortage_qty: required_qty - available_qty,
                            });
                        }
                    }
                }
            }

            if !material_shortages.is_empty() {
                warnings.push(format!(
                    "物料不足：{} 种组件短缺",
                    material_shortages.len()
                ));
            }

            validations.push(ReleaseValidation {
                plan_item_id: item.id,
                product_id: item.product_id,
                has_routing,
                has_published_bom,
                routing_id: routing_detail.map(|rd| rd.routing.id),
                warnings,
                material_shortages,
            });
        }

        Ok(validations)
    }

    async fn release_to_work_orders(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. 预校验
        let validations = self.pre_validate(ctx, db, plan_id).await?;

        let mut successful = Vec::new();
        let mut failed = Vec::new();

        let work_order_svc = new_work_order_service(self.pool.clone());

        // 2. 逐个创建 + release（单工单失败不影响其余）
        for (item, _validation) in items.iter().zip(validations.iter()) {
            let scheduled_start = item.scheduled_start;
            let scheduled_end = item.scheduled_end;

            // 创建工单
            let create_result = work_order_svc.create(
                ctx, db,
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
            ).await;

            let wo_id = match create_result {
                Ok(id) => id,
                Err(e) => {
                    failed.push(BatchFailure {
                        index: item.id as i32,
                        error: e,
                    });
                    continue;
                }
            };

            // 立即 release
            let wo = match work_order_svc.find_by_id(ctx, db, wo_id).await {
                Ok(wo) => wo,
                Err(e) => {
                    failed.push(BatchFailure {
                        index: item.id as i32,
                        error: e,
                    });
                    continue;
                }
            };

            match work_order_svc.release(ctx, db, wo_id, wo.version).await {
                Ok(()) => {
                    // 更新 PlanItem 状态 → Released
                    let _ = ProductionPlanRepo::update_item_status(
                        &mut *db, item.id,
                        super::super::enums::PlanItemStatus::Released,
                    ).await;

                    if let Ok(released_wo) = work_order_svc.find_by_id(ctx, db, wo_id).await {
                        successful.push(released_wo);
                    }
                }
                Err(e) => {
                    failed.push(BatchFailure {
                        index: item.id as i32,
                        error: e,
                    });
                }
            }
        }

        // 3. 更新计划状态
        if !successful.is_empty() {
            ProductionPlanRepo::update_status(&mut *db, plan_id, PlanStatus::InProgress)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        let total = items.len() as i32;
        Ok(BatchReleaseResult {
            plan_id,
            successful_work_orders: successful,
            failed_items: failed,
            validations,
            total,
        })
    }
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -50`
Expected: 无 error。注意 `pre_validate` 中的 inventory 查询可能需要调整——如果 `inventories` 表结构不同，需要修改 SQL。

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/production_plan/
git commit -m "feat(plan): pre_validate + release_to_work_orders one-click release

- pre_validate: check Routing, BOM, material availability
- release_to_work_orders: create + release per item, failure isolation
- Update PlanItem status on success
- Return ReleaseValidation with warnings and material shortages"
```

---

### Task 3: 超额生产容差控制

**Files:**
- Modify: `abt-core/src/master_data/product/model.rs` (ProductMeta)
- Modify: `abt-core/src/mes/production_batch/implt.rs` (confirm_routing_step)

- [ ] **Step 1: ProductMeta 增加 over_completion_tolerance**

在 `ProductMeta` struct 中添加：

```rust
    /// 超额完工容差百分比（默认 5%）
    #[serde(default = "default_tolerance")]
    pub over_completion_tolerance: Option<rust_decimal::Decimal>,
```

在 `ProductMeta` 定义之前添加辅助函数：

```rust
fn default_tolerance() -> Option<rust_decimal::Decimal> {
    Some(rust_decimal::Decimal::from_str_exact("0.05").unwrap())
}
```

- [ ] **Step 2: 在 confirm_routing_step 中增加容差校验**

在 `abt-core/src/mes/production_batch/implt.rs` 的 `confirm_routing_step()` 方法中，在最后工序报工逻辑处增加校验。

找到报工完成量校验的位置（通常在更新 completed_qty 之前），添加：

```rust
        // 超额生产容差校验（最后工序）
        let routings = WorkOrderRoutingRepo::list_by_work_order(&mut *db, batch.work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let max_step: i32 = routings.iter().map(|r| r.step_no).max().unwrap_or(0);
        let is_last_step = req.step_no == max_step;

        if is_last_step {
            let total_completed: rust_decimal::Decimal = routings.iter()
                .filter(|r| r.step_no == req.step_no)
                .map(|r| r.completed_qty)
                .sum::<rust_decimal::Decimal>()
                + req.completed_qty;
            let total_defect: rust_decimal::Decimal = routings.iter()
                .filter(|r| r.step_no == req.step_no)
                .map(|r| r.defect_qty)
                .sum::<rust_decimal::Decimal>()
                + req.defect_qty;

            let total_reported = total_completed + total_defect;

            // 获取容差
            let tolerance = get_over_completion_tolerance(&self.pool, ctx, db, batch.work_order_id).await?;
            let max_allowed = batch.batch_qty * (rust_decimal::Decimal::ONE + tolerance);

            if total_reported > max_allowed {
                return Err(DomainError::BusinessRule(
                    format!(
                        "报工量 {} 超出计划量 {} 的允许偏差范围 (容差 {}%)",
                        total_reported,
                        batch.batch_qty,
                        tolerance * rust_decimal::Decimal::ONE_HUNDRED
                    ),
                ));
            }
        }
```

- [ ] **Step 3: 添加容差获取辅助函数**

在 `production_batch/implt.rs` 中添加辅助函数：

```rust
/// 获取超额完工容差（优先级：工单 → 产品 → 系统默认 5%）
async fn get_over_completion_tolerance(
    pool: &PgPool,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    work_order_id: i64,
) -> Result<rust_decimal::Decimal> {
    let default = rust_decimal::Decimal::from_str_exact("0.05").unwrap();

    // 获取工单的产品
    let wo = new_work_order_service(pool.clone())
        .find_by_id(ctx, db, work_order_id).await?;

    // 获取产品 meta 中的容差设置
    let product = new_product_service(pool.clone())
        .get(ctx, db, wo.product_id).await?;

    Ok(product.meta.over_completion_tolerance.unwrap_or(default))
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/master_data/product/model.rs abt-core/src/mes/production_batch/implt.rs
git commit -m "feat(batch): add over-completion tolerance check in confirm_routing_step

- Default 5% tolerance, configurable per product via ProductMeta
- Reject last-step reporting that exceeds planned_qty × (1 + tolerance)"
```

---

### Task 4: 事件发布（release + unrelease）

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`

- [ ] **Step 1: 在 release() 末尾添加事件发布**

在 release() 方法的审计日志之前添加：

```rust
        // 发布事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                crate::shared::event_bus::EventPublishRequest {
                    event_type: crate::shared::enums::event::DomainEventType::WorkOrderReleased,
                    aggregate_type: "WorkOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "product_id": work_order.product_id,
                        "planned_qty": work_order.planned_qty,
                        "bom_snapshot_id": bom_snapshot_id,
                        "has_routing": routing_detail.is_some(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;
```

需要在文件顶部添加 use：
```rust
use crate::shared::event_bus::{new_domain_event_bus, EventPublishRequest};
use crate::shared::enums::event::DomainEventType;
```

注意：需要确认 `DomainEventType` 中是否已有 `WorkOrderReleased` 变体。如果没有，需要在 `shared/enums/event.rs` 中添加。

- [ ] **Step 2: 在 unrelease() 末尾添加事件发布**

在 unrelease() 审计日志之前添加：

```rust
        // 发布事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                crate::shared::event_bus::EventPublishRequest {
                    event_type: crate::shared::enums::event::DomainEventType::WorkOrderUnreleased,
                    aggregate_type: "WorkOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "product_id": work_order.product_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;
```

- [ ] **Step 3: 检查 DomainEventType 枚举**

检查 `abt-core/src/shared/enums/event.rs`，如果缺少以下变体，需要添加：

```rust
    WorkOrderReleased,
    WorkOrderUnreleased,
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -40`
Expected: 无 error

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/work_order/implt.rs abt-core/src/shared/enums/
git commit -m "feat(mes): publish domain events on release/unrelease

- WorkOrderReleased: triggers downstream QMS and FMS processing
- WorkOrderUnreleased: triggers inventory reservation release"
```

---

### Task 5: 验证阶段 3

- [ ] **Step 1: 全量 clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: 无 error

- [ ] **Step 2: 运行测试**

Run: `cargo test -p abt-core 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: 最终 commit**

```bash
git add -A
git commit -m "feat(mes): phase 3 complete — one-click release + pre-validation + tolerance + events"
```
