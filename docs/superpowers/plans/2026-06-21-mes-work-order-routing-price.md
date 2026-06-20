# 工单工序计件单价维护 + 工序删除 + wage 冻结 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在工单详情页工序列表行内编辑计件单价（>0 必填）与删除工序（零报工时可删、删后 step_no 重排），并冻结报工工资到 `work_reports.wage_amount`，使报工自动带价且历史工资不可漂移。

**Architecture:** 三层改动——(1) `abt-core` repo 新增 6 个 `WorkOrderRoutingRepo` 方法 + 2 个 `ProductionBatchService` trait 方法（带状态/报工守卫与审计）；(2) `abt-web` 新增 2 个 TypedPath + handler 返回行/表片段；(3) `mes_order_detail::tab_routing` 渲染可编辑单价列与删除列。另加 migration 062 给 `work_reports` 加 `wage_amount` 列，报工写入冻结值、读取不再实时重算。

**Tech Stack:** Rust 2024 / axum + axum_extra::TypedPath / sqlx（原始 SQL）/ Maud / HTMX 2.0.10 / Hyperscript / rust_decimal / async-trait / PostgreSQL。

## Global Constraints

- **沟通用中文；commit message 中文**，结尾加 `Co-Authored-By: Claude <noreply@anthropic.com>`
- **不要 `cargo run` 启动服务**（已在运行）；验证用 `cargo clippy` 和 `cargo test`
- **代码导航用 `lsp`**，禁止用文本搜索代替 LSP 查定义/引用
- **跨模块只走 Service trait / Model**；mes 模块内部（work_order / production_batch / work_report）可互调各自 Repo
- **共享服务用按需工厂**：`new_xxx_service(self.pool.clone())`，struct 只持 `PgPool`
- **错误禁止静默丢弃**：全用 `?` / `map_err`；DomainError 用 `validation` / `business_rule` / `not_found` 构造
- **所有 TypedPath**，禁止硬编码 URL 字符串
- **样式 100% UnoCSS 原子类**，禁止 `style=""` 内联（`<col>` 例外）；禁止改 `static/app.css`
- **测试为 DB 集成测试**，放 `abt-web/tests/`，用 `common::TestApp`（连真实 dev DB），串行 + 独立实体隔离。`abt-core` 无单测 harness，不要新建纯单测
- **migration 编号续 062**，纯 SQL，幂等（`ADD COLUMN IF NOT EXISTS`）
- 金额精度：`NUMERIC(20,4)` / `Decimal`；数量 `DECIMAL(18,6)`

## 参考文件（实现前必读）

- `abt-core/src/mes/production_batch/repo.rs` — `WorkOrderRoutingRepo`（270 行起）、`WorkReportRepo`（590 行起）、`InsertWorkReportParams`、`WorkReportRow`
- `abt-core/src/mes/production_batch/service.rs` — `ProductionBatchService` trait
- `abt-core/src/mes/production_batch/implt.rs` — `confirm_routing_step`（174 行起，含 wage 计算 ~222 行）
- `abt-core/src/mes/work_report/model.rs` — `WorkReport`、`ReportListItem`、`WageDetail`
- `abt-core/src/mes/work_report/implt.rs` — `calculate_wage`（71 行起，实时重算 wage）
- `abt-core/src/mes/work_report/repo.rs` — `list` 查询（193 行起，已 JOIN work_order_routings）
- `abt-core/src/shared/enums/cost.rs`、`abt-core/src/shared/audit_log/`（`new_audit_log_service`、`RecordAuditLogReq`、`AuditAction`）
- `abt-core/src/shared/types/error.rs` — `DomainError::validation/business_rule/not_found`
- `abt-web/src/routes/mes_order.rs` — TypedPath + `router()` 范式
- `abt-web/src/pages/mes_order_detail.rs` — `get_order_detail`、`tab_routing`（558 行起）
- `abt-web/tests/common/mod.rs` — `TestApp` harness
- `abt-web/tests/mes_flow_e2e.rs` — 工单创建+下达的 seeding 写法（复用其 helper）
- 设计文档：`docs/superpowers/specs/2026-06-21-mes-work-order-routing-price-design.md`

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/migrations/062_work_report_wage_amount.sql` | 给 work_reports 加 wage_amount + 回填 | 新建 |
| `abt-core/src/mes/production_batch/repo.rs` | 6 个新 repo 方法；`WorkReportRow`/`InsertWorkReportParams` 加 wage_amount；INSERT/RETURNING 加列 | 改 |
| `abt-core/src/mes/production_batch/service.rs` | trait 加 `update_routing_unit_price` / `delete_routing` | 改 |
| `abt-core/src/mes/production_batch/implt.rs` | 两个方法实现 + 守卫 + 审计；`confirm_routing_step` 把 wage_amount 传入 INSERT | 改 |
| `abt-core/src/mes/work_report/model.rs` | `WorkReport`、`ReportListItem` 加字段 | 改 |
| `abt-core/src/mes/work_report/repo.rs` | `find_by_id` 等查询 SELECT 加 wage_amount；`list` SELECT 加 `wr.routing_id` | 改 |
| `abt-core/src/mes/work_report/implt.rs` | `calculate_wage` 改读冻结值 | 改 |
| `abt-web/src/routes/mes_order.rs` | 2 个新 TypedPath + 注册 | 改 |
| `abt-web/src/pages/mes_order_detail.rs` | 2 个 handler + `tab_routing` 改可编辑/删除 + `get_order_detail` 算 reported 集合 | 改 |
| `abt-web/tests/mes_routing_price.rs` | 集成测试：改价/删除守卫与效果、wage 冻结 | 新建 |

---

### Task 1: WorkOrderRoutingRepo 新增 6 个方法

**Files:**
- Modify: `abt-core/src/mes/production_batch/repo.rs`（`impl WorkOrderRoutingRepo` 块内，紧随 `get_by_work_order_and_step` 之后，约 329 行后）

**Interfaces:**
- Produces（供 Task 2 消费，签名固定）：
  - `async fn get_by_id(executor: &mut sqlx::postgres::PgConnection, routing_id: i64) -> Result<Option<WorkOrderRouting>>`
  - `async fn update_unit_price(executor: &mut sqlx::postgres::PgConnection, routing_id: i64, unit_price: Decimal) -> Result<()>`
  - `async fn delete(executor: &mut sqlx::postgres::PgConnection, routing_id: i64) -> Result<()>`
  - `async fn renumber_steps(executor: &mut sqlx::postgres::PgConnection, work_order_id: i64) -> Result<()>`
  - `async fn has_report(executor: &mut sqlx::postgres::PgConnection, routing_id: i64) -> Result<bool>`
  - `async fn has_any_report(executor: &mut sqlx::postgres::PgConnection, work_order_id: i64) -> Result<bool>`

> `Decimal` 来自 `rust_decimal::Decimal`；`Result` 是 `abt_core::shared::types::Result`（文件顶部已 `use`）。`WorkOrderRouting::from_row` 已存在，照 `get_by_work_order_and_step` 用法复用。

- [ ] **Step 1: 写失败测试**

新建 `abt-web/tests/mes_routing_price.rs`（本任务先只测 repo，后续 Task 补 service/UI 用例）。复用 `mes_flow_e2e.rs` 里建工单+下达的 helper 拿到一个 `work_order_id` 与其 `WorkOrderRouting` 列表。先写最小骨架：

```rust
mod common;

use abt_core::mes::production_batch::repo::WorkOrderRoutingRepo;
use abt_core::mes::production_batch::{ProductionBatchService, new_production_batch_service};
use rust_decimal::Decimal;
use sqlx::postgres::PgConnection;

async fn first_routing_id(state: &abt_web::state::AppState, wo_id: i64) -> i64 {
    let svc = state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = state.pool().acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    rs[0].id
}

#[tokio::test]
async fn repo_update_unit_price_persists() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await; // 复用 mes_flow_e2e helper
    let rid = first_routing_id(&app.state, wo_id).await;
    let mut conn = app.state.pool().acquire().await.unwrap();
    WorkOrderRoutingRepo::update_unit_price(&mut conn, rid, Decimal::new(125, 2))
        .await
        .unwrap();
    let after = WorkOrderRoutingRepo::get_by_id(&mut conn, rid).await.unwrap().unwrap();
    assert_eq!(after.unit_price, Some(Decimal::new(125, 2))); // 1.25
}

#[tokio::test]
async fn repo_has_report_false_before_reporting() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let mut conn = app.state.pool().acquire().await.unwrap();
    assert!(!WorkOrderRoutingRepo::has_report(&mut conn, rid).await.unwrap());
    assert!(!WorkOrderRoutingRepo::has_any_report(&mut conn, wo_id).await.unwrap());
}
```

> 若 `common` 没有现成的 `seed_released_work_order` / `admin_service_ctx` / `pool()` 访问器，**实现期读 `abt-web/tests/mes_flow_e2e.rs` 与 `common/mod.rs`**，把那里建工单+下达的调用抽成 `common::seed_released_work_order(&app) -> wo_id` 并加 `pub fn pool(&self) -> &PgPool`（若 AppState 未暴露）。这是本计划唯一需要按既有 helper 对齐的地方——先读再写，不要臆造签名。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p abt-web --test mes_routing_price 2>&1 | tail -20`
Expected: 编译失败——`update_unit_price` / `get_by_id` / `has_report` / `has_any_report` 方法不存在。

- [ ] **Step 3: 实现 repo 方法**

在 `abt-core/src/mes/production_batch/repo.rs` 的 `impl WorkOrderRoutingRepo` 块、`get_by_work_order_and_step` 方法之后插入：

```rust
    /// 按 id 查找工序（带 work_order_id 用于越权校验）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<Option<WorkOrderRouting>> {
        let row = sqlx::query(
            r#"
            SELECT id, work_order_id, step_no, process_name, work_center_id,
                   standard_time, standard_cost, unit_price, allowed_loss_rate,
                   planned_qty, is_outsourced, is_inspection_point
            FROM work_order_routings
            WHERE id = $1
            "#,
        )
        .bind(routing_id)
        .fetch_optional(&mut *executor)
        .await?;
        row.map(|r| Ok(WorkOrderRouting::from_row(&r)?)).transpose()
    }

    /// 更新单条工序计件单价
    pub async fn update_unit_price(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
        unit_price: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE work_order_routings SET unit_price = $2 WHERE id = $1
            "#,
        )
        .bind(routing_id)
        .bind(unit_price)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 删除单条工序
    pub async fn delete(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<()> {
        sqlx::query(r#"DELETE FROM work_order_routings WHERE id = $1"#)
            .bind(routing_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// 删除后重排：剩余工序 step_no 压成 1..N 连续
    pub async fn renumber_steps(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            WITH ordered AS (
                SELECT id, ROW_NUMBER() OVER (ORDER BY step_no) AS new_no
                FROM work_order_routings
                WHERE work_order_id = $1
            )
            UPDATE work_order_routings wor
            SET step_no = ordered.new_no::int
            FROM ordered
            WHERE wor.id = ordered.id
            "#,
        )
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 该工序是否已有报工记录（改价逐行守卫）
    pub async fn has_report(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM work_reports WHERE routing_id = $1)"#,
        )
        .bind(routing_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(exists.0)
    }

    /// 该工单是否有任意报工记录（删除全局守卫）
    pub async fn has_any_report(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM work_reports wr
                JOIN work_order_routings wor ON wor.id = wr.routing_id
                WHERE wor.work_order_id = $1
            )
            "#,
        )
        .bind(work_order_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(exists.0)
    }
```

确认文件顶部已 `use rust_decimal::Decimal;`（若没有则加）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p abt-web --test mes_routing_price -- repo_ 2>&1 | tail -20`
Expected: 两个 repo 测试 PASS。

- [ ] **Step 5: clippy**

Run: `cargo clippy -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/mes/production_batch/repo.rs abt-web/tests/mes_routing_price.rs abt-web/tests/common/mod.rs
git commit -m "feat(mes): WorkOrderRoutingRepo 新增 get_by_id/update_unit_price/delete/renumber_steps/has_report/has_any_report

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: ProductionBatchService 改价 + 删除（守卫 + 审计）

**Files:**
- Modify: `abt-core/src/mes/production_batch/service.rs`（trait 加 2 方法）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（实现 2 方法 + 审计）

**Interfaces:**
- Consumes: Task 1 的 6 个 repo 方法；`new_work_order_service(self.pool.clone()).find_by_id(ctx, db, wo_id)`（既有工厂，取工单状态）；`new_audit_log_service(self.pool.clone()).record(ctx, db, RecordAuditLogReq{...})`；`DomainError::{validation,business_rule,not_found}`
- Produces（供 Task 4 handler 消费）：
  - `async fn update_routing_unit_price(&self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64, routing_id: i64, unit_price: Decimal) -> Result<WorkOrderRouting>`
  - `async fn delete_routing(&self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64, routing_id: i64) -> Result<()>`

- [ ] **Step 1: 写失败测试（service 守卫）**

在 `abt-web/tests/mes_routing_price.rs` 追加（`Decimal`、`ProductionBatchService`、`new_production_batch_service` 已在 Task 1 引入或此处补 `use abt_core::shared::types::DomainError;`）：

```rust
use abt_core::shared::types::DomainError;

async fn price_svc(state: &abt_web::state::AppState) -> impl ProductionBatchService {
    state.production_batch_service()
}

#[tokio::test]
async fn service_update_price_rejects_zero() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let svc = app.state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();
    let err = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_id, rid, Decimal::ZERO)
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn service_update_price_ok_then_persists() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let svc = app.state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();
    let updated = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_id, rid, Decimal::new(3, 0))
        .await
        .unwrap();
    assert_eq!(updated.unit_price, Some(Decimal::new(3, 0)));
    assert_eq!(updated.id, rid);
}

#[tokio::test]
async fn service_update_price_rejects_cross_order() {
    let app = common::TestApp::new().await;
    let wo_a = common::seed_released_work_order(&app).await;
    let wo_b = common::seed_released_work_order(&app).await;
    let rid_a = first_routing_id(&app.state, wo_a).await;
    let svc = app.state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();
    let err = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_b, rid_a, Decimal::new(3, 0))
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::NotFound { .. }), "got {err:?}");
}

#[tokio::test]
async fn service_delete_renumbers_and_blocks_last() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await; // 假设 seed 产出 ≥2 道工序
    let svc = app.state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();
    let mut rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    // 删第一道 → 剩余重排
    svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap();
    rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    for (i, r) in rs.iter().enumerate() {
        assert_eq!(r.step_no as usize, i + 1);
    }
    // 删到只剩一道时拒绝
    while rs.len() > 1 {
        let id = rs[0].id;
        svc.delete_routing(&ctx, &mut conn, wo_id, id).await.unwrap();
        rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    }
    let err = svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }), "got {err:?}");
}
```

> `seed_released_work_order` 须产出**至少 2 道工序**的工单；若现有 helper 只产 1 道，在 `common` 里调整 seed 用的 routing 使其 ≥2 道（读 mes_flow_e2e.rs 确认 routing 来源）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p abt-web --test mes_routing_price -- service_ 2>&1 | tail -20`
Expected: 编译失败——trait 上没有这两个方法。

- [ ] **Step 3: trait 加方法**

在 `abt-core/src/mes/production_batch/service.rs` 的 `ProductionBatchService` trait 中、`list_routings` 之后加：

```rust
    async fn update_routing_unit_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        unit_price: rust_decimal::Decimal,
    ) -> Result<WorkOrderRouting>;
    async fn delete_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
    ) -> Result<()>;
```

- [ ] **Step 4: 实现两个方法（implt.rs）**

在 `abt-core/src/mes/production_batch/implt.rs` 顶部 `use` 区确保有：
```rust
use rust_decimal::Decimal;
use crate::shared::audit_log::{new_audit_log_service, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::DomainError;
use crate::mes::work_order::{new_work_order_service, WorkOrderStatus};
use super::repo::WorkOrderRoutingRepo;
```
（按 `lsp` 确认这些路径在当前文件是否已 import，缺啥补啥，勿重复 import。）

在 `impl ProductionBatchService for ProductionBatchServiceImpl` 块内（`confirm_routing_step` 之后）加：

```rust
    async fn update_routing_unit_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        unit_price: Decimal,
    ) -> Result<WorkOrderRouting> {
        // 守卫 1：单价 > 0
        if unit_price <= Decimal::ZERO {
            return Err(DomainError::validation("计件单价必须大于 0"));
        }
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 守卫 2/3：routing 存在且属于该工单
        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }

        // 守卫 2：工单状态 ∈ {Released, InProduction}
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *tx, work_order_id)
            .await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许修改工序单价"));
        }

        // 守卫 4：该工序未报工（事务内复查防并发）
        if WorkOrderRoutingRepo::has_report(&mut *tx, routing_id).await? {
            return Err(DomainError::business_rule("该工序已报工，单价不可修改"));
        }

        let old_price = routing.unit_price;
        WorkOrderRoutingRepo::update_unit_price(&mut *tx, routing_id, unit_price).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, &mut *tx, RecordAuditLogReq {
                entity_type: "WorkOrderRouting",
                entity_id: routing_id,
                action: AuditAction::Update,
                changes: Some(format!(
                    "unit_price: {:?} → {:?}",
                    old_price, unit_price
                )),
                context: Some(format!("work_order_id={}", work_order_id)),
            })
            .await?;

        let updated = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;

        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(updated)
    }

    async fn delete_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }

        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *tx, work_order_id)
            .await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许删除工序"));
        }

        // 守卫：整单零报工
        if WorkOrderRoutingRepo::has_any_report(&mut *tx, work_order_id).await? {
            return Err(DomainError::business_rule("工单已有报工记录，不可删除工序"));
        }
        // 守卫：至少保留一道
        let remaining: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint FROM work_order_routings WHERE work_order_id = $1"#,
        )
        .bind(work_order_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if remaining <= 1 {
            return Err(DomainError::business_rule("至少保留一道工序"));
        }

        WorkOrderRoutingRepo::delete(&mut *tx, routing_id).await?;
        WorkOrderRoutingRepo::renumber_steps(&mut *tx, work_order_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, &mut *tx, RecordAuditLogReq {
                entity_type: "WorkOrderRouting",
                entity_id: routing_id,
                action: AuditAction::Delete,
                changes: Some(format!(
                    "删除工序 {} {}",
                    routing.step_no, routing.process_name
                )),
                context: Some(format!("work_order_id={}", work_order_id)),
            })
            .await?;

        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }
```

> `RecordAuditLogReq` 的字段名以 `abt-core/src/shared/audit_log/model.rs` 为准（实现期用 `lsp` hover 确认 `changes`/`context` 字段类型是 `Option<String>` 还是 `Option<serde_json::Value>`，按实际调整——expense/implt.rs:148 用的是 `changes: None, context: None`，照其类型填）。

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p abt-web --test mes_routing_price -- service_ 2>&1 | tail -25`
Expected: 4 个 service 测试 PASS。

- [ ] **Step 6: clippy**

Run: `cargo clippy -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

- [ ] **Step 7: 提交**

```bash
git add abt-core/src/mes/production_batch/service.rs abt-core/src/mes/production_batch/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): ProductionBatchService 改价/删工序 + 守卫 + 审计

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: ReportListItem 增加 routing_id（供 UI 推导已报工集合）

**Files:**
- Modify: `abt-core/src/mes/work_report/model.rs:72-92`（`ReportListItem` 加字段）
- Modify: `abt-core/src/mes/work_report/repo.rs`（`list` 的 SELECT 加 `wr.routing_id`）

**Interfaces:**
- Produces: `ReportListItem.routing_id: i64`（供 Task 5 `get_order_detail` 构造 `reported_routing_ids`）

- [ ] **Step 1: 写失败测试**

`abt-web/tests/mes_routing_price.rs` 追加（先借一条已有报工的数据；若无，先跑一次报工再查 list）：

```rust
use abt_core::mes::work_report::{WorkReportService, ReportListFilter};

#[tokio::test]
async fn report_list_item_carries_routing_id() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_work_order_with_one_report(&app).await; // 复用/补 helper：建单→下达→报工一道
    let svc = app.state.work_report_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();
    let page = svc
        .list(&ctx, &mut conn,
              ReportListFilter { work_order_id: Some(wo_id), ..Default::default() },
              1, 50)
        .await
        .unwrap();
    assert!(page.items.iter().all(|i| i.routing_id > 0));
}
```

> `seed_work_order_with_one_report` 若 `common` 无，则在 `common` 里基于 `seed_released_work_order` + 一次 `confirm_routing_step` 组合实现（读 `mes_flow_e2e.rs` 的报工调用照抄）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p abt-web --test mes_routing_price -- report_list_item 2>&1 | tail`
Expected: 编译失败——`ReportListItem` 无 `routing_id` 字段。

- [ ] **Step 3: model 加字段**

`abt-core/src/mes/work_report/model.rs` 的 `ReportListItem` 结构体（72 行起）在 `routing_id` 逻辑对应处加字段。该 struct 现无 `routing_id`，加在 `product_id` 后：

```rust
pub struct ReportListItem {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub batch_id: i64,
    pub routing_id: i64,            // ← 新增
    pub product_id: i64,
    // ...其余字段不变
```

- [ ] **Step 4: SELECT 加列**

`abt-core/src/mes/work_report/repo.rs` 的 `list` 查询（193 行起的 `data_sql`），在 SELECT 列表里 `wr.id, wr.doc_number` 之后加 `wr.routing_id,`（用 `lsp`/读确认列序与 `query_as::<ReportListItem>` 的 FromRow 顺序无关——sqlx 按列名匹配）。

```sql
SELECT wr.id, wr.doc_number, wr.routing_id,
       wr.work_order_id, wr.batch_id, ...
```

> 同时检查 repo.rs 内其它返回 `ReportListItem` 的查询是否共用同一段 SQL；若 `list` 是唯一来源则只改它。用 `lsp find references` 于 `ReportListItem` 确认。

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p abt-web --test mes_routing_price -- report_list_item 2>&1 | tail`
Expected: PASS。

- [ ] **Step 6: clippy + 提交**

Run: `cargo clippy -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

```bash
git add abt-core/src/mes/work_report/model.rs abt-core/src/mes/work_report/repo.rs abt-web/tests/mes_routing_price.rs abt-web/tests/common/mod.rs
git commit -m "feat(mes): ReportListItem 增加 routing_id 供工单详情推导已报工集合

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Web 层 — 改价/删除 TypedPath + Handler + 路由注册

**Files:**
- Modify: `abt-web/src/routes/mes_order.rs`（2 个 TypedPath + 注册）
- Modify: `abt-web/src/pages/mes_order_detail.rs`（2 个 handler + 片段渲染函数）

**Interfaces:**
- Consumes: Task 2 的 `update_routing_unit_price` / `delete_routing`；Task 1 `list_routings`（已有）
- Produces: `POST /admin/mes/orders/{order_id}/routings/{routing_id}/price`、`POST /admin/mes/orders/{order_id}/routings/{routing_id}/delete` 两个端点（供 Task 5 UI 调用）

- [ ] **Step 1: 写失败测试（HTTP 层）**

`abt-web/tests/mes_routing_price.rs` 追加（用 `TestApp` 的 router 发请求；参考 `mes_pages.rs` / `mes_order.rs` 既有测试的请求构造写法）：

```rust
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn http_update_price_endpoint_returns_row_fragment() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let router = common::router_with_session(&app).await; // 既有 helper，照 mes_pages.rs
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/mes/orders/{}/routings/{}/price", wo_id, rid))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("unit_price=2.50"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn http_update_price_rejects_zero_with_4xx() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let router = common::router_with_session(&app).await;
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/mes/orders/{}/routings/{}/price", wo_id, rid))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("unit_price=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_client_error(), "got {}", resp.status());
}
```

> 请求构造与 session 注入的精确 helper 名以 `abt-web/tests/common/mod.rs` 与 `mes_pages.rs` 为准（`router_with_session` / `oneshot` 等照既有用法）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p abt-web --test mes_routing_price -- http_ 2>&1 | tail`
Expected: 编译失败或 404（路由未注册）。

- [ ] **Step 3: 加 TypedPath + 注册**

`abt-web/src/routes/mes_order.rs`，在 `OrderSplitPath` 之后加：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/price")]
pub struct OrderRoutingPricePath {
    pub order_id: i64,
    pub routing_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/delete")]
pub struct OrderRoutingDeletePath {
    pub order_id: i64,
    pub routing_id: i64,
}
```

在 `router()` 的 `.route(OrderSplitPath::PATH, ...)` 之后加：

```rust
        .route(OrderRoutingPricePath::PATH, post(mes_order_detail::update_routing_price))
        .route(OrderRoutingDeletePath::PATH, post(mes_order_detail::delete_routing))
```

- [ ] **Step 4: 加 handler**

`abt-web/src/pages/mes_order_detail.rs`。顶部 `use` 加：
```rust
use crate::routes::mes_order::{OrderRoutingPricePath, OrderRoutingDeletePath};
use abt_core::shared::types::DomainError;
```

在 `split_order` handler 之后加：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct RoutingPriceForm {
    pub unit_price: rust_decimal::Decimal,
}

/// 行内修改工序计件单价，返回更新后的该行 <tr>
#[require_permission("WORK_ORDER", "update")]
pub async fn update_routing_price(
    path: OrderRoutingPricePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingPriceForm>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let updated = svc
        .update_routing_unit_price(
            &service_ctx, &mut conn,
            path.order_id, path.routing_id, form.unit_price,
        )
        .await?;
    // 该行未报工（守卫已保证），渲染为可编辑行
    Ok(Html(routing_row_fragment(&updated, false).into_string()))
}

/// 删除工序，返回重排后的整个 <tbody>
#[require_permission("WORK_ORDER", "update")]
pub async fn delete_routing(
    path: OrderRoutingDeletePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.delete_routing(&service_ctx, &mut conn, path.order_id, path.routing_id).await?;
    let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    Ok(Html(routing_tbody_fragment(&routings, false).into_string()))
}
```

并提取两个渲染函数（Task 5 会把 `tab_routing` 改为复用它们）。**本步骤先放占位实现**，Task 5 完成真正的 `<tr>`/`<tbody>` 标记：

```rust
/// 渲染单行 <tr>（reported=false → 可编辑单价）
fn routing_row_fragment(r: &WorkOrderRouting, reported: bool) -> Markup {
    html! { tr { td colspan="9" { "TODO Task5" } } } // Task 5 替换
}

/// 渲染整个 <tbody>（order_has_report=false → 含删除列）
fn routing_tbody_fragment(routings: &[WorkOrderRouting], order_has_report: bool) -> Markup {
    html! { tbody { tr { td colspan="9" { "TODO Task5" } } } } // Task 5 替换
}
```

> 这两个函数在 Task 5 被实化；本任务只要让端点可编译、可路由、返回 200/4xx。`TODO Task5` 标记是计划内临时态，Task 5 必须消除。

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p abt-web --test mes_routing_price -- http_ 2>&1 | tail`
Expected: 两个 HTTP 测试 PASS（200 与 4xx）。

- [ ] **Step 6: clippy + 提交**

Run: `cargo clippy -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

```bash
git add abt-web/src/routes/mes_order.rs abt-web/src/pages/mes_order_detail.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 工单工序改价/删除端点（handler 占位渲染，UI 见下个提交）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: UI — tab_routing 可编辑单价列 + 删除列

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs`（`get_order_detail` 算 reported 集合；`tab_routing` 改造；实化 Task 4 的两个 fragment 函数）

**Interfaces:**
- Consumes: Task 3 的 `ReportListItem.routing_id`；Task 4 的两个端点路径
- Produces: 工单详情「工序明细」tab 的可编辑单价 + 删除交互

- [ ] **Step 1: 写失败测试（UI 文案/结构）**

`abt-web/tests/mes_routing_price.rs` 追加——请求详情页 HTML，断言未报工工单的页面里出现 `name="unit_price"` 的 input 和「删除」按钮：

```rust
#[tokio::test]
async fn detail_page_shows_editable_price_and_delete_when_unreported() {
    let app = common::TestApp::new().await;
    let wo_id = common::seed_released_work_order(&app).await;
    let router = common::router_with_session(&app).await;
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/admin/mes/orders/{}", wo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = common::body_string(resp).await; // 既有 helper，照 mes_pages.rs
    assert!(body.contains(r#"name="unit_price""#), "单价 input 缺失");
    assert!(body.contains("/delete"), "删除端点缺失");
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p abt-web --test mes_routing_price -- detail_page_shows 2>&1 | tail`
Expected: FAIL——当前详情页单价是只读文本。

- [ ] **Step 3: 改 get_order_detail 算 reported 集合**

`mes_order_detail.rs` 的 `get_order_detail`（80 行起），在 `reports` 取完后加：

```rust
    // 已报工 routing_id 集合 + 整单是否有报工
    let reported_routing_ids: std::collections::HashSet<i64> =
        reports.iter().map(|r| r.routing_id).collect();
    let order_has_report = !reports.is_empty();
```

把这两个值传入 `order_detail_page` → `tab_routing`。修改 `order_detail_page` 与 `tab_routing` 签名（加两个参数），调用点同步更新。

- [ ] **Step 4: 实化两个 fragment 函数 + 改造 tab_routing**

把 Task 4 的占位 `routing_row_fragment` / `routing_tbody_fragment` 替换为真实渲染，并让 `tab_routing` 复用：

```rust
fn routing_row_fragment(r: &WorkOrderRouting, order_has_report: bool) -> Markup {
    let reported_step = order_has_report; // 整单有报工 → 所有行只读（删/改都锁）
    html! {
        tr {
            td class="font-mono tabular-nums" { (r.step_no) }
            td { strong { (r.process_name.as_str()) } }
            td class="font-mono tabular-nums" {
                @if let Some(wc) = r.work_center_id { "#" (wc) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.planned_qty)) }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if let Some(t) = r.standard_time { (crate::utils::fmt_qty(t)) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if let Some(c) = r.standard_cost { "¥" (crate::utils::fmt_qty(c)) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if reported_step {
                    @if let Some(p) = r.unit_price { "¥" (crate::utils::fmt_qty(p)) } @else { "—" }
                } @else {
                    input class="w-[88px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm bg-white outline-none focus:border-accent"
                        type="number" step="any" min="0.000001" name="unit_price"
                        value=(r.unit_price.map(|p| p.to_string()).unwrap_or_default())
                        hx-post=(OrderRoutingPricePath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-trigger="change"
                        hx-target="closest tr"
                        hx-swap="outerHTML"
                        hx-disabled-elt="this";
                }
            }
            td {
                @if r.is_outsourced { span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-warn-bg text-warn" { "委外" } } @else { "—" }
            }
            td {
                @if r.is_inspection_point {
                    span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-accent-bg text-accent" { "报检" }
                } @else { "—" }
            }
            td class="text-center" {
                @if !reported_step {
                    button class="text-muted hover:text-danger cursor-pointer border-none bg-transparent p-1"
                        title="删除该工序"
                        hx-post=(OrderRoutingDeletePath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-confirm="删除该工序并重排后续工序号？"
                        hx-target="closest tbody"
                        hx-swap="outerHTML"
                        hx-disabled-elt="this" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                } @else { "—" }
            }
        }
    }
}

fn routing_tbody_fragment(routings: &[WorkOrderRouting], order_has_report: bool) -> Markup {
    html! {
        tbody {
            @for r in routings {
                (routing_row_fragment(r, order_has_report))
            }
            @if routings.is_empty() {
                tr { td colspan="10" class="text-center text-muted text-sm" { "暂无工序明细（工单未下达或无工艺路线）" } }
            }
        }
    }
}
```

> `icon::trash_icon` 若 `components::icon` 没有，则用现有任一删除图标函数（`lsp` 查 `icon::` 下含 `delete`/`trash`/`x` 的函数），或直接内联一个 SVG（`<svg>` 用 `maud::PreEscaped` 包裹的写法参考其它页面）。优先复用已有图标函数。

把原 `tab_routing` 表体的 `<tbody>...</tbody>` 替换为 `(routing_tbody_fragment(routings, order_has_report))`，表头 `<th>` 增加「操作」列（共 10 列，`colspan` 同步改 10）。

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p abt-web --test mes_routing_price -- detail_page_shows 2>&1 | tail`
Expected: PASS。

- [ ] **Step 6: clippy + 提交**

Run: `cargo clippy -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error（含消除 Task 4 的 `TODO Task5`）。

```bash
git add abt-web/src/pages/mes_order_detail.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 工单详情工序列表行内改单价 + 删除工序（零报工时可删/重排）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: wage_amount 冻结 — migration + 落库 + 读取

**Files:**
- Create: `abt-core/migrations/062_work_report_wage_amount.sql`
- Modify: `abt-core/src/mes/work_report/model.rs`（`WorkReport` 加 `wage_amount`）
- Modify: `abt-core/src/mes/production_batch/repo.rs`（`WorkReportRow`、`InsertWorkReportParams`、`insert_or_get_existing` 的 INSERT/RETURNING/bind）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（`confirm_routing_step` 把算出的 wage_amount 传入 `InsertWorkReportParams`）
- Modify: `abt-core/src/mes/work_report/repo.rs`（所有 `SELECT ... FROM work_reports` 的查询列加 `wage_amount`：`find_by_id`、`list_by_worker_and_date_range`、`list_by_date_range` 等）
- Modify: `abt-core/src/mes/work_report/implt.rs`（`calculate_wage` 改读 `report.wage_amount`）

**Interfaces:**
- Produces: `work_reports.wage_amount NUMERIC(20,4)`；`WorkReport.wage_amount`、`WageDetail.wage_amount` 取冻结值

- [ ] **Step 1: 写失败测试**

`abt-web/tests/mes_routing_price.rs` 追加——报工后查 wage summary，断言金额等于「报工时单价 × 合格量」且**之后改工序单价不影响该笔**：

```rust
use abt_core::mes::work_report::{WorkReportService, DateRange};
use chrono::NaiveDate;

#[tokio::test]
async fn wage_is_frozen_at_report_time() {
    let app = common::TestApp::new().await;
    let (wo_id, worker_id, report_date) = common::seed_work_order_with_one_report(&app).await;
    let svc = app.state.production_batch_service();
    let wrk_svc = app.state.work_report_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();

    // 报工时金额
    let before = wrk_svc.calculate_wage(&ctx, &mut conn, worker_id,
        DateRange { from: report_date, to: report_date }).await.unwrap();

    // 之后改该工序单价（注：已报工 → 守卫应拒；改其它未报工工序不影响本笔）
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    if let Some(other) = rs.iter().find(|r| !common::routing_has_report(&app, r.id).await) {
        let _ = svc.update_routing_unit_price(&ctx, &mut conn, wo_id, other.id,
            rust_decimal::Decimal::new(999, 0)).await;
    }

    let after = wrk_svc.calculate_wage(&ctx, &mut conn, worker_id,
        DateRange { from: report_date, to: report_date }).await.unwrap();
    assert_eq!(before.total_amount, after.total_amount, "报工后改价不应影响历史工资");
    assert!(before.total_amount > rust_decimal::Decimal::ZERO);
}
```

> `common::routing_has_report` 若无，用 `WorkOrderRoutingRepo::has_report` 直查（已在 Task 1）。`seed_work_order_with_one_report` 返回 `(wo_id, worker_id, report_date)`。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p abt-web --test mes_routing_price -- wage_is_frozen 2>&1 | tail`
Expected: FAIL（当前实时重算，改价后金额会变；且 `calculate_wage` 还没读冻结字段）。

- [ ] **Step 3: 写 migration**

`abt-core/migrations/062_work_report_wage_amount.sql`：

```sql
-- 报工冻结工资：报工落库时写入，避免后续改工序单价导致历史工资漂移
ALTER TABLE work_reports ADD COLUMN IF NOT EXISTS wage_amount NUMERIC(20,4) NOT NULL DEFAULT 0;

-- 回填历史报工（与运行时公式一致：(completed_qty + affect_wage的不良) × unit_price）
UPDATE work_reports wr
SET wage_amount = (wr.completed_qty +
        CASE WHEN wr.defect_reason IS NOT NULL
             AND wr.defect_reason IN (1)  -- affect_wage 的 DefectReason 取值，实现期按 enums::DefectReason::affect_wage 集合核对
             THEN wr.defect_qty ELSE 0 END)
    * COALESCE((SELECT wor.unit_price FROM work_order_routings wor WHERE wor.id = wr.routing_id), 0)
WHERE wr.wage_amount = 0;
```

> `defect_reason IN (1)` 的取值集合：实现期读 `abt-core/src/mes/enums.rs` 里 `DefectReason::affect_wage()` 返回 true 的成员对应 i16 值，把集合写全（如 `IN (1,3)` 等）。`affect_wage` 的运行时判定见 `work_report/implt.rs:111-114`。

- [ ] **Step 4: model 加字段**

`abt-core/src/mes/work_report/model.rs` 的 `WorkReport`（6 行起）加：
```rust
    pub wage_amount: rust_decimal::Decimal,
```
（放在 `work_hours` 之后。）

- [ ] **Step 5: repo — WorkReportRow / InsertWorkReportParams / INSERT**

`abt-core/src/mes/production_batch/repo.rs`：
- `WorkReportRow` 结构体（约 575-588 行）加 `pub wage_amount: Decimal,`
- `InsertWorkReportParams<'_>` 结构体加 `pub wage_amount: Decimal,`
- `insert_or_get_existing`（596 行起）：INSERT 列表加 `wage_amount`（放 `operator_id` 前），VALUES 加对应 `$N` 占位（注意 `$N` 编号顺延），`.bind(params.wage_amount)`；RETURNING 列表加 `wage_amount`。
- `WorkReportRow::from_row`（若有手动映射）加 `wage_amount: row.try_get("wage_amount")?`；若是 `sqlx::FromRow` derive 则自动。

- [ ] **Step 6: confirm_routing_step 传入 wage_amount**

`abt-core/src/mes/production_batch/implt.rs` 的 `confirm_routing_step`，定位构造 `InsertWorkReportParams { ... }` 处，把已算出的 `wage_amount`（约 222 行算出、`StepConfirmationResult.wage_amount` 同源）填入 `wage_amount` 字段。用 `lsp` 跳到 `InsertWorkReportParams` 构造点确认字段。

- [ ] **Step 7: work_report 查询 SELECT 加列**

`abt-core/src/mes/work_report/repo.rs`：把所有 `SELECT id, doc_number, work_order_id, batch_id, routing_id, ...` 的列清单加 `wage_amount`（`find_by_id`、`list_by_worker_and_date_range`、`list_by_date_range` 等返回 `WorkReport` 的查询，行号见 17/38/64/92/117 附近）。用 `lsp find references` 于 `WorkReport` 确认全部来源。

- [ ] **Step 8: calculate_wage 改读冻结值**

`abt-core/src/mes/work_report/implt.rs` 的 `calculate_wage`（102-130 行循环体），把：
```rust
            let wage_amount = (report.completed_qty + non_operator_defect_qty) * unit_price;
```
改为：
```rust
            let wage_amount = report.wage_amount;  // 报工时冻结，不再实时重算
```
保留 `unit_price`（仍用于 `WageDetail.unit_price` 展示）与 `non_operator_defect_qty` 计算（展示用，若不再需要可移除——优先保留以减少改动面）。

- [ ] **Step 9: 应用 migration + 运行测试**

Run: 应用 migration 到 dev DB（按项目既有方式，如 `sqlx migrate run` 或重启服务自动迁移——确认 `abt-core` migration 应用机制）。
Run: `cargo test -p abt-web --test mes_routing_price -- wage_is_frozen 2>&1 | tail`
Expected: PASS。

- [ ] **Step 10: clippy + 全量测试 + 提交**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price 2>&1 | tail -20`
Expected: 全部 PASS，无 error。

```bash
git add abt-core/migrations/062_work_report_wage_amount.sql abt-core/src/mes/work_report/model.rs abt-core/src/mes/production_batch/repo.rs abt-core/src/mes/production_batch/implt.rs abt-core/src/mes/work_report/repo.rs abt-core/src/mes/work_report/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 报工冻结 wage_amount 到 work_reports，消除工资实时重算漂移

migration 062 加列+回填；confirm_routing_step 落库冻结值；calculate_wage
改读冻结值；历史报工不受后续改工序单价影响。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: 端到端串联验证 + 设计文档同步

**Files:**
- Modify: `abt-web/tests/mes_routing_price.rs`（补一条完整串联用例）
- Modify: `docs/uml-design/04-mes.html`（同步 `ProductionBatchService` 新接口）

- [ ] **Step 1: 写串联测试**

`abt-web/tests/mes_routing_price.rs` 追加——建单→下达→改价→报工→断言带价与冻结→再改价被拒：

```rust
#[tokio::test]
async fn full_flow_edit_price_report_freeze_lock() {
    let app = common::TestApp::new().await;
    let (wo_id, batch_id, routing_id, worker_id) = common::seed_released_work_order_full(&app).await;
    let svc = app.state.production_batch_service();
    let ctx = common::admin_service_ctx();
    let mut conn = app.state.pool().acquire().await.unwrap();

    // 1. 改价
    svc.update_routing_unit_price(&ctx, &mut conn, wo_id, routing_id,
        rust_decimal::Decimal::new(5, 0)).await.unwrap();

    // 2. 报工（confirm_routing_step 内部冻结 wage）
    let req = common::step_confirm_req(worker_id, rust_decimal::Decimal::new(10, 0));
    let result = svc.confirm_routing_step(&ctx, &mut conn, batch_id, 1, req).await.unwrap();
    assert_eq!(result.wage_amount, rust_decimal::Decimal::new(50, 0)); // 10 × 5

    // 3. 报工后改该工序价 → 拒
    let err = svc.update_routing_unit_price(&ctx, &mut conn, wo_id, routing_id,
        rust_decimal::Decimal::new(9, 0)).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }));

    // 4. 删除该工单任意工序 → 拒（已有报工）
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let err = svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }));
}
```

> `seed_released_work_order_full` / `step_confirm_req` 复用 mes_flow_e2e.rs 的既有构造；返回 `(wo_id, batch_id, routing_id, worker_id)`。

- [ ] **Step 2: 运行 + 修至 PASS**

Run: `cargo test -p abt-web --test mes_routing_price 2>&1 | tail -25`
Expected: 全部 PASS。失败则按失败点回到对应 Task 修。

- [ ] **Step 3: 同步 uml-design**

更新 `docs/uml-design/04-mes.html` 中 `ProductionBatchService` 接口列表，加入 `update_routing_unit_price` / `delete_routing` 两个方法签名（与 service.rs 一致），并在 work_reports 模型标注 `wage_amount` 字段。

- [ ] **Step 4: clippy 全量 + 提交**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error|^warning: unused" | head`
Expected: 无新增 error。

```bash
git add abt-web/tests/mes_routing_price.rs docs/uml-design/04-mes.html
git commit -m "test(mes): 工序改价→报工冻结→锁 完整串联 + 同步 uml-design 接口

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review（写计划后自检）

**1. Spec 覆盖**：
- §5.1 repo 6 方法 → Task 1 ✓
- §5.2 service 2 方法 + 守卫 + 审计 → Task 2 ✓
- §5.3 路由 + handler → Task 4 ✓
- §5.4 UI（reported 集合、可编辑单价列、删除列）→ Task 3（routing_id）+ Task 5 ✓
- §6 错误处理（并发复查/越权/负数/删最后一条）→ Task 2 守卫 ✓
- §7 测试 → Task 1/2/3/4/5/6/7 分布 ✓
- §8 wage 冻结 → Task 6 ✓
- §9 uml-design 同步 → Task 7 Step 3 ✓
- §10/§11 是范围声明，无需任务 ✓

**2. 占位符扫描**：Task 4 的 `TODO Task5` 是计划内显式临时态，Task 5 Step 4 明确消除。其余 `common::seed_*` / `common::router_with_session` / `icon::trash_icon` / `DefectReason IN(...)` 均给出「读既有文件确认」的具体指引，非空洞 TODO。

**3. 类型一致性**：repo 6 方法签名（Task 1）与 Task 2 调用一致；service 2 方法签名（Task 2 trait）与 Task 4 handler 调用一致；`ReportListItem.routing_id`（Task 3）与 Task 5 `reported_routing_ids` 一致；`wage_amount` 字段（Task 6）在 model/repo/service/read 四处一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-mes-work-order-routing-price.md`. Two execution options:

**1. Subagent-Driven (recommended)** — 每个 Task 派一个新 subagent，任务间我来 review，迭代快、上下文干净。
**2. Inline Execution** — 在当前会话用 executing-plans 批量执行，带检查点 review。

Which approach?
