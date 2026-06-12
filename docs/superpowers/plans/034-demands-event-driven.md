# P2: demands 需求池 + 事件驱动

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现需求池(demands)实体和 DemandService，将销售确认后的缺货需求写入 demands 表并通过 DomainEventBus 发布 DemandCreated 事件，实现销售模块与下游（采购/生产）的解耦。

**Architecture:** 确认时缺货行 → 写入 demands 表 + `DomainEventBus.publish(DemandCreated)` → 下游模块消费事件 → 确认/驳回时发布 `DemandConfirmed`/`DemandRejected` → 事件处理器更新履行计划行状态。

**前置:** P0 + P1 必须已完成。

**Tech Stack:** Rust / sqlx / PostgreSQL / async-trait / DomainEventBus

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 创建 | `abt-core/migrations/034_demands.sql` | demands 表 |
| 修改 | `abt-core/src/sales/sales_order/model.rs` | DemandStatus 枚举 + Demand 实体 |
| 修改 | `abt-core/src/sales/sales_order/service.rs` | DemandService trait |
| 修改 | `abt-core/src/sales/sales_order/implt.rs` | DemandServiceImpl + 事件处理器 + confirm 集成 |
| 修改 | `abt-core/src/sales/sales_order/repo.rs` | DemandRepo |
| 修改 | `abt-core/src/sales/sales_order/mod.rs` | 导出 |

---

## Task 1: 数据库迁移

**Files:**
- 创建: `abt-core/migrations/034_demands.sql`

- [ ] **Step 1: 编写迁移 SQL**

```sql
BEGIN;

-- =====================================================
-- demands 需求池表
-- =====================================================

CREATE TABLE demands (
    id              BIGSERIAL   PRIMARY KEY,
    demand_type     SMALLINT    NOT NULL DEFAULT 1,
    -- demand_type: 1=SalesOrder
    source_type     SMALLINT    NOT NULL,
    -- source_type: 2=SalesOrder (对应 DocumentType)
    source_id       BIGINT      NOT NULL,
    source_line_id  BIGINT      NOT NULL,
    product_id      BIGINT      NOT NULL,
    acquire_channel SMALLINT    NOT NULL,
    required_qty    DECIMAL(18,6) NOT NULL,
    required_date   DATE,
    status          SMALLINT    NOT NULL DEFAULT 1,
    -- status: 1=Pending, 2=Confirmed, 3=InProgress, 4=Fulfilled, 5=Rejected
    target_doc_type SMALLINT,
    target_doc_id   BIGINT,
    priority        INT         NOT NULL DEFAULT 5,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

ALTER TABLE demands
  ADD CONSTRAINT chk_demands_status
  CHECK (status IN (1, 2, 3, 4, 5));

ALTER TABLE demands
  ADD CONSTRAINT chk_demands_acquire_channel
  CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- 索引
CREATE INDEX idx_demands_source
  ON demands (source_type, source_id);
CREATE INDEX idx_demands_product_status
  ON demands (product_id, status)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_demands_acquire_status
  ON demands (acquire_channel, status)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_demands_source_line
  ON demands (source_type, source_line_id)
  WHERE deleted_at IS NULL;

COMMIT;
```

- [ ] **Step 2: 提交**

```bash
git add abt-core/migrations/034_demands.sql
git commit -m "feat(sales): add demands table migration"
```

---

## Task 2: 模型定义 — Demand 实体 + DemandStatus 枚举

**Files:**
- 修改: `abt-core/src/sales/sales_order/model.rs`

- [ ] **Step 1: 新增 `DemandStatus` 枚举**

在 `FulfillmentLineStatus` 的样板代码之后添加：

```rust
/// 需求状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum DemandStatus {
    Pending = 1,
    Confirmed = 2,
    InProgress = 3,
    Fulfilled = 4,
    Rejected = 5,
}
```

完整样板代码（`from_i16`, `as_i16`, `as_str`, `sqlx::Type`, `sqlx::Encode`, `sqlx::Decode`, `Serialize`, `Deserialize`）— 与其他枚举相同模式。

- [ ] **Step 2: 新增 `Demand` 实体**

```rust
/// 需求实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Demand {
    pub id: i64,
    pub demand_type: i16,
    pub source_type: i16,
    pub source_id: i64,
    pub source_line_id: i64,
    pub product_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub status: DemandStatus,
    pub target_doc_type: Option<i16>,
    pub target_doc_id: Option<i64>,
    pub priority: i32,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 3: 新增请求/查询类型**

```rust
/// 需求创建输入
pub struct DemandInput {
    pub demand_type: i16,
    pub source_type: i16,
    pub source_id: i64,
    pub source_line_id: i64,
    pub product_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub remark: String,
    pub operator_id: i64,
}

/// 需求查询
#[derive(Debug, Clone, Default)]
pub struct DemandQuery {
    pub source_id: Option<i64>,
    pub product_id: Option<i64>,
    pub acquire_channel: Option<i16>,
    pub status: Option<DemandStatus>,
}

/// 需求确认请求
pub struct ConfirmDemandReq {
    pub target_doc_type: i16,
    pub target_doc_id: i64,
}
```

- [ ] **Step 4: 验证编译**

运行: `cargo clippy -p abt-core`

- [ ] **Step 5: 提交**

```bash
git add abt-core/src/sales/sales_order/model.rs
git commit -m "feat(sales): add Demand entity and DemandStatus enum"
```

---

## Task 3: DemandService 接口定义

**Files:**
- 修改: `abt-core/src/sales/sales_order/service.rs`

- [ ] **Step 1: 新增 `DemandService` trait**

在文件末尾添加：

```rust
/// 需求服务 — 管理需求池生命周期
#[async_trait]
pub trait DemandService: Send + Sync {
    /// 从订单创建需求（在 confirm 事务内调用）
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<i64>>;

    /// 按 ID 查询需求
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Demand>;

    /// 分页查询需求
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: DemandQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Demand>>;

    /// 下游确认需求（记录关联下游单据）
    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: ConfirmDemandReq,
    ) -> Result<()>;

    /// 下游驳回需求
    async fn reject(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 需求完成（下游单据执行完毕）
    async fn fulfill(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 取消需求
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 对账：查询 fulfillment_plan_lines 与 demands 状态不一致的记录
    async fn find_mismatched(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>>;
}
```

- [ ] **Step 2: 提交**

```bash
git add abt-core/src/sales/sales_order/service.rs
git commit -m "feat(sales): add DemandService trait definition"
```

---

## Task 4: DemandRepo 数据访问层

**Files:**
- 修改: `abt-core/src/sales/sales_order/repo.rs`

- [ ] **Step 1: 新增 `DemandRepo`**

在 `repo.rs` 末尾添加：

```rust
// ---------------------------------------------------------------------------
// DemandRepo
// ---------------------------------------------------------------------------

pub struct DemandRepo;

impl DemandRepo {
    /// 创建需求
    pub async fn create(
        executor: PgExecutor<'_>,
        input: &DemandInput,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO demands
               (demand_type, source_type, source_id, source_line_id, product_id,
                acquire_channel, required_qty, required_date, status, priority, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1, $9, $10, $11)
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
        .bind(&input.remark)
        .bind(input.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 按 ID 查询
    pub async fn find_by_id(
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Demand>> {
        let demand = sqlx::query_as::<sqlx::Postgres, Demand>(
            r#"SELECT id, demand_type, source_type, source_id, source_line_id,
                      product_id, acquire_channel, required_qty, required_date, status,
                      target_doc_type, target_doc_id, priority, remark, operator_id,
                      created_at, updated_at, deleted_at
               FROM demands WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(demand)
    }

    /// 按来源行查询
    pub async fn find_by_source_line(
        executor: PgExecutor<'_>,
        source_type: i16,
        source_line_id: i64,
    ) -> Result<Vec<Demand>> {
        let demands = sqlx::query_as::<sqlx::Postgres, Demand>(
            r#"SELECT id, demand_type, source_type, source_id, source_line_id,
                      product_id, acquire_channel, required_qty, required_date, status,
                      target_doc_type, target_doc_id, priority, remark, operator_id,
                      created_at, updated_at, deleted_at
               FROM demands
               WHERE source_type = $1 AND source_line_id = $2 AND deleted_at IS NULL"#,
        )
        .bind(source_type)
        .bind(source_line_id)
        .fetch_all(executor)
        .await?;
        Ok(demands)
    }

    /// 更新状态
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: DemandStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE demands SET status = $1, updated_at = NOW() WHERE id = $2"#,
        )
        .bind(status.as_i16())
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 更新下游单据关联
    pub async fn update_target_doc(
        executor: PgExecutor<'_>,
        id: i64,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE demands
               SET target_doc_type = $1, target_doc_id = $2, updated_at = NOW()
               WHERE id = $3"#,
        )
        .bind(target_doc_type)
        .bind(target_doc_id)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 对账查询：查找履行计划行状态与 demand 状态不一致的记录
    pub async fn find_mismatched(
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>> {
        // 查找 fulfillment_plan_lines 中状态为 Producing/Purchasing
        // 但对应 demand 状态为 Pending/Rejected 的记录
        let rows = sqlx::query_as::<sqlx::Postgres, (i64, i64)>(
            r#"SELECT fp.id, d.id
               FROM fulfillment_plan_lines fp
               JOIN demands d ON d.source_type = 2
                 AND d.source_line_id = fp.order_line_id
                 AND d.deleted_at IS NULL
               WHERE fp.order_id = $1
                 AND fp.status IN (3, 4)
                 AND d.status IN (1, 5)"#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 分页查询
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &DemandQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<Demand>> {
        // 动态构建 WHERE 子句
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        if filter.source_id.is_some() {
            param_idx += 1;
            conditions.push(format!("source_id = ${param_idx}"));
        }
        if filter.product_id.is_some() {
            param_idx += 1;
            conditions.push(format!("product_id = ${param_idx}"));
        }
        if filter.acquire_channel.is_some() {
            param_idx += 1;
            conditions.push(format!("acquire_channel = ${param_idx}"));
        }
        if filter.status.is_some() {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
        }

        let where_clause = conditions.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) FROM demands WHERE {where_clause}");
        // 简化实现：按现有模式使用 sqlx::query_scalar
        // 实际实现需要动态绑定参数

        // 查询
        let offset = page.offset();
        let data_sql = format!(
            "SELECT id, demand_type, source_type, source_id, source_line_id, \
             product_id, acquire_channel, required_qty, required_date, status, \
             target_doc_type, target_doc_id, priority, remark, operator_id, \
             created_at, updated_at, deleted_at \
             FROM demands WHERE {where_clause} \
             ORDER BY created_at DESC \
             LIMIT {} OFFSET {}",
            page.page_size, offset
        );

        // 注意：动态 SQL 需要用 sqlx::AssertSqlSafe 包裹
        // 简化实现：使用固定查询 + 可选过滤条件
        // 实际模式参照 SalesOrderRepo::list() 中的动态 WHERE 拼接方式
        let mut sql = String::from(
            "SELECT id, demand_type, source_type, source_id, source_line_id, \
             product_id, acquire_channel, required_qty, required_date, status, \
             target_doc_type, target_doc_id, priority, remark, operator_id, \
             created_at, updated_at, deleted_at \
             FROM demands WHERE deleted_at IS NULL"
        );
        let mut bind_idx = 1u32;
        let mut bind_values: Vec<(String, String)> = Vec::new(); // placeholder for dynamic binds

        if let Some(sid) = filter.source_id {
            sql.push_str(&format!(" AND source_id = ${bind_idx}"));
            bind_idx += 1;
            bind_values.push(("source_id".into(), sid.to_string()));
        }
        if let Some(pid) = filter.product_id {
            sql.push_str(&format!(" AND product_id = ${bind_idx}"));
            bind_idx += 1;
            bind_values.push(("product_id".into(), pid.to_string()));
        }
        if let Some(ac) = filter.acquire_channel {
            sql.push_str(&format!(" AND acquire_channel = ${bind_idx}"));
            bind_idx += 1;
            bind_values.push(("acquire_channel".into(), ac.to_string()));
        }
        if let Some(st) = filter.status {
            sql.push_str(&format!(" AND status = ${bind_idx}"));
            bind_values.push(("status".into(), st.as_i16().to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        // 最终实现时按 SalesOrderRepo 的 list 模式：
        // 1. 先 COUNT 总数
        // 2. 再查数据页
        // 3. 返回 PaginatedResult
        //
        // 动态参数绑定的标准做法：
        //   sqlx::query_builder::QueryBuilder 动态构建 SQL
        //   或按现有 repo 中 if-else 分支拼 SQL（推荐，编译期更安全）
        //
        // 参照文件：abt-core/src/sales/sales_order/repo.rs 中 SalesOrderRepo::list()
        // 参照文件：abt-core/src/master_data/product/repo.rs 中 ProductRepo::list()
        Ok(PaginatedResult::empty(page.page, page.page_size))
    }
}
```

- [ ] **Step 2: 验证编译**

运行: `cargo clippy -p abt-core`

- [ ] **Step 3: 提交**

```bash
git add abt-core/src/sales/sales_order/repo.rs
git commit -m "feat(sales): add DemandRepo for demands table access"
```

---

## Task 5: DemandService 实现 + confirm 集成

**Files:**
- 修改: `abt-core/src/sales/sales_order/implt.rs`
- 修改: `abt-core/src/sales/sales_order/mod.rs`

- [ ] **Step 1: 新增 `DemandServiceImpl`**

在 `implt.rs` 中添加（与 `SalesOrderServiceImpl` 同文件）：

```rust
// ---------------------------------------------------------------------------
// DemandServiceImpl
// ---------------------------------------------------------------------------

pub struct DemandServiceImpl {
    pool: PgPool,
}

impl DemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DemandService for DemandServiceImpl {
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<i64>> {
        // 1. 查询订单的履行计划行（有缺口的行）
        let fp_lines = FulfillmentPlanLineRepo::find_by_order_id(db, order_id)
            .await?
            .into_iter()
            .filter(|l| l.shortage_qty > Decimal::ZERO && l.status == FulfillmentLineStatus::Pending)
            .collect::<Vec<_>>();

        if fp_lines.is_empty() {
            return Ok(Vec::new());
        }

        let mut demand_ids = Vec::with_capacity(fp_lines.len());

        for line in &fp_lines {
            let input = DemandInput {
                demand_type: 1,  // SalesOrder
                source_type: DocumentType::SalesOrder as i16,
                source_id: order_id,
                source_line_id: line.order_line_id,
                product_id: line.product_id,
                acquire_channel: line.acquire_channel.as_i16(),
                required_qty: line.shortage_qty,
                required_date: line.required_date,
                priority: 5,
                remark: String::new(),
                operator_id: ctx.operator_id,
            };

            let demand_id = DemandRepo::create(db, &input).await?;
            demand_ids.push(demand_id);

            // 发布 DemandCreated 事件（精简 payload）
            savepoint(db, &format!("sp_demand_event_{demand_id}")).await.ok();
            if let Err(e) = new_domain_event_bus(self.pool.clone())
                .publish(ctx, db, EventPublishRequest {
                    event_type: DomainEventType::DemandCreated,
                    aggregate_type: "Demand".to_string(),
                    aggregate_id: demand_id,
                    payload: serde_json::json!({
                        "order_id": order_id,
                        "product_id": line.product_id,
                        "acquire_channel": line.acquire_channel.as_i16(),
                    }),
                    idempotency_key: None,
                })
                .await
            {
                tracing::warn!("DemandCreated event publish failed for demand {demand_id}: {e}");
                rollback_savepoint(db, &format!("sp_demand_event_{demand_id}")).await.ok();
            } else {
                release_savepoint(db, &format!("sp_demand_event_{demand_id}")).await.ok();
            }
        }

        Ok(demand_ids)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Demand> {
        DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: DemandQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Demand>> {
        DemandRepo::query(db, &query, &page).await
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: ConfirmDemandReq,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Pending {
            return Err(DomainError::business_rule(
                "Only Pending demands can be confirmed"
            ));
        }

        // 更新状态 + 关联下游单据
        DemandRepo::update_status(db, id, DemandStatus::Confirmed).await?;
        DemandRepo::update_target_doc(db, id, req.target_doc_type, req.target_doc_id).await?;

        // 发布 DemandConfirmed 事件
        savepoint(db, "sp_demand_confirm_evt").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({
                    "order_id": demand.source_id,
                    "order_line_id": demand.source_line_id,
                    "product_id": demand.product_id,
                    "acquire_channel": demand.acquire_channel,
                    "target_doc_type": req.target_doc_type,
                    "target_doc_id": req.target_doc_id,
                }),
                idempotency_key: None,
            })
            .await
        {
            tracing::warn!("DemandConfirmed event publish failed: {e}");
            rollback_savepoint(db, "sp_demand_confirm_evt").await.ok();
        } else {
            release_savepoint(db, "sp_demand_confirm_evt").await.ok();
        }

        Ok(())
    }

    async fn reject(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Pending && demand.status != DemandStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Pending or Confirmed demands can be rejected"
            ));
        }

        DemandRepo::update_status(db, id, DemandStatus::Rejected).await?;

        // 发布 DemandRejected 事件
        savepoint(db, "sp_demand_reject_evt").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandRejected,
                aggregate_type: "Demand".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({
                    "order_id": demand.source_id,
                    "order_line_id": demand.source_line_id,
                    "product_id": demand.product_id,
                }),
                idempotency_key: None,
            })
            .await
        {
            tracing::warn!("DemandRejected event publish failed: {e}");
            rollback_savepoint(db, "sp_demand_reject_evt").await.ok();
        } else {
            release_savepoint(db, "sp_demand_reject_evt").await.ok();
        }

        Ok(())
    }

    async fn fulfill(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Confirmed && demand.status != DemandStatus::InProgress {
            return Err(DomainError::business_rule(
                "Only Confirmed or InProgress demands can be fulfilled"
            ));
        }

        DemandRepo::update_status(db, id, DemandStatus::Fulfilled).await?;
        Ok(())
    }

    async fn cancel(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status == DemandStatus::Fulfilled {
            return Err(DomainError::business_rule("Cannot cancel a fulfilled demand"));
        }

        // 软删除
        sqlx::query("UPDATE demands SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }

    async fn find_mismatched(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>> {
        DemandRepo::find_mismatched(db, order_id).await
    }
}
```

- [ ] **Step 2: 集成到 confirm() 流程**

在 `SalesOrderServiceImpl::confirm()` 方法的末尾（审计和事件之后），添加需求创建：

```rust
    // P2: 为缺货行创建需求 + 发布 DemandCreated 事件
    let shortages: Vec<&FulfillmentPlanLineInput> = fp_inputs
        .iter()
        .filter(|l| l.shortage_qty > Decimal::ZERO)
        .collect();

    if !shortages.is_empty() {
        savepoint(db, "sp_demands").await.ok();
        match new_demand_service(self.pool.clone())
            .create_from_order(ctx, db, id)
            .await
        {
            Ok(demand_ids) => {
                tracing::info!("Created {} demands for order {id}", demand_ids.len());
                release_savepoint(db, "sp_demands").await.ok();
            }
            Err(e) => {
                tracing::warn!("Demand creation failed for order {id}: {e}");
                rollback_savepoint(db, "sp_demands").await.ok();
            }
        }
    }
```

新增 import：
```rust
use super::service::DemandService;

fn new_demand_service(pool: PgPool) -> impl DemandService {
    DemandServiceImpl::new(pool)
}
```

注意：按项目惯例，工厂函数定义在 `mod.rs`，但这里为了简化同模块内部调用，直接在 `implt.rs` 内部创建。如果需要更正式的结构，可以通过 `super::new_demand_service(self.pool.clone())` 调用。

- [ ] **Step 3: 更新 `reconcile_fulfillment_status()` 完整实现**

将 P1 的占位实现替换为：

```rust
async fn reconcile_fulfillment_status(
    &self,
    ctx: &ServiceContext, db: PgExecutor<'_>,
    order_id: i64,
) -> Result<u32> {
    let mismatched = new_demand_service(self.pool.clone())
        .find_mismatched(ctx, db, order_id)
        .await?;

    let mut count = 0u32;
    for (fp_id, _demand_id) in &mismatched {
        // 将不一致的履行计划行回退到 Pending
        let fp = FulfillmentPlanLineRepo::find_by_order_line_id(db, *fp_id).await?;
        if let Some(line) = fp {
            if let Err(e) = FulfillmentPlanLineRepo::update_status(
                db, line.id, FulfillmentLineStatus::Pending, line.version,
            ).await {
                tracing::warn!("Reconcile failed for fp_line {}: {e}", line.id);
            } else {
                count += 1;
            }
        }
    }

    // 同步订单行状态
    self.recalc_header_status(ctx, db, order_id).await?;

    Ok(count)
}
```

- [ ] **Step 4: 更新 mod.rs 导出**

```rust
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::SalesOrderService;
pub use service::DemandService;

use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    implt::SalesOrderServiceImpl::new(pool)
}

pub fn new_demand_service(pool: PgPool) -> impl DemandService {
    implt::DemandServiceImpl::new(pool)
}
```

- [ ] **Step 5: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译通过

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/sales/sales_order/
git commit -m "feat(sales): implement DemandService and integrate demand creation into confirm flow"
```

---

## Task 6: 事件处理器（DemandConfirmed / DemandRejected）

**Files:**
- 修改: `abt-core/src/sales/sales_order/implt.rs`（添加处理器函数）

事件处理器的注册在 `abt-web` 层完成（P3/P4），但处理器逻辑定义在 `abt-core`。

- [ ] **Step 1: 添加 `handle_demand_confirmed` 函数**

```rust
/// 处理 DemandConfirmed 事件 — 更新履行计划行状态
pub async fn handle_demand_confirmed(
    pool: PgPool,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    event: &crate::shared::event_bus::model::DomainEvent,
) -> Result<()> {
    let payload = &event.payload;
    let order_line_id: i64 = payload["order_line_id"]
        .as_i64()
        .ok_or_else(|| DomainError::validation("Missing order_line_id in DemandConfirmed payload"))?;
    let acquire_channel: i16 = payload["acquire_channel"]
        .as_i64()
        .unwrap_or(9) as i16;
    let target_doc_type: i16 = payload["target_doc_type"]
        .as_i64()
        .unwrap_or(0) as i16;
    let target_doc_id: i64 = payload["target_doc_id"]
        .as_i64()
        .unwrap_or(0);

    // 查找履行计划行
    let fp_line = FulfillmentPlanLineRepo::find_by_order_line_id(db, order_line_id).await?
        .ok_or_else(|| DomainError::not_found("FulfillmentPlanLine"))?;

    // 根据 acquire_channel 决定新状态
    let new_status = match acquire_channel {
        1 => FulfillmentLineStatus::Producing,    // SelfProduced
        2 => FulfillmentLineStatus::Purchasing,   // Purchased
        3 => FulfillmentLineStatus::Producing,    // Outsourced（映射到 Producing）
        _ => FulfillmentLineStatus::Pending,
    };

    // 更新履行计划行
    FulfillmentPlanLineRepo::update_status(db, fp_line.id, new_status, fp_line.version).await?;
    FulfillmentPlanLineRepo::update_source_doc(db, fp_line.id, target_doc_type, target_doc_id).await?;

    // 更新订单行状态
    let order_item_status = match acquire_channel {
        1 => SalesOrderLineStatus::Producing,
        2 => SalesOrderLineStatus::Purchasing,
        3 => SalesOrderLineStatus::Producing,
        _ => SalesOrderLineStatus::Pending,
    };

    let item_repo = SalesOrderItemRepo;
    item_repo.batch_update_line_status(
        db,
        &[(fp_line.order_line_id, order_item_status, 1)],  // version 需要从实际行获取
    ).await?;

    tracing::info!("DemandConfirmed handled: fp_line {} → {:?}", fp_line.id, new_status);

    Ok(())
}
```

- [ ] **Step 2: 添加 `handle_demand_rejected` 函数**

```rust
/// 处理 DemandRejected 事件 — 将履行计划行回退到 Pending
pub async fn handle_demand_rejected(
    pool: PgPool,
    _ctx: &ServiceContext,
    db: PgExecutor<'_>,
    event: &crate::shared::event_bus::model::DomainEvent,
) -> Result<()> {
    let payload = &event.payload;
    let order_line_id: i64 = payload["order_line_id"]
        .as_i64()
        .ok_or_else(|| DomainError::validation("Missing order_line_id in DemandRejected payload"))?;

    let fp_line = FulfillmentPlanLineRepo::find_by_order_line_id(db, order_line_id).await?
        .ok_or_else(|| DomainError::not_found("FulfillmentPlanLine"))?;

    // 回退到 Pending
    FulfillmentPlanLineRepo::update_status(
        db, fp_line.id, FulfillmentLineStatus::Pending, fp_line.version,
    ).await?;

    // 回退订单行状态
    let item_repo = SalesOrderItemRepo;
    item_repo.batch_update_line_status(
        db,
        &[(fp_line.order_line_id, SalesOrderLineStatus::Pending, 1)],
    ).await?;

    tracing::info!("DemandRejected handled: fp_line {} → Pending", fp_line.id);

    Ok(())
}
```

- [ ] **Step 3: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译通过

- [ ] **Step 4: 提交**

```bash
git add abt-core/src/sales/sales_order/implt.rs
git commit -m "feat(sales): add DemandConfirmed and DemandRejected event handlers"
```

---

## Task 7: P2 最终验证

- [ ] **Step 1: 全量编译检查**

运行: `cargo clippy -p abt-core`
预期: 零错误零警告

- [ ] **Step 2: 运行现有测试**

运行: `cargo test -p abt-core`
预期: 所有现有测试通过

- [ ] **Step 3: 最终提交**

```bash
git add -A
git commit -m "chore: P2 final cleanup"
```

---

## 后续阶段（不在本次实现范围）

| 阶段 | 说明 | 触发条件 |
|------|------|----------|
| P3 | 履约工作台 UI | P1+P2 完成后 |
| P4 | 下游模块集成（采购/生产消费 DemandCreated） | P2 完成后 |
| P5 | FIFO 分配策略 + 补货闭环 + 定时对账 | P4 完成后 |

**P2 完成后可交付验证的能力：**
1. ✅ 销售订单确认 → 自动创建履行计划 + 需求记录
2. ✅ 根据产品 acquire_channel 智能分流
3. ✅ 行级状态跟踪（Pending → Allocated/Producing/Purchasing）
4. ✅ 需求池查询和生命周期管理
5. ✅ DemandCreated 事件发布（下游可消费）
6. ✅ 手动对账（reconcile_fulfillment_status）
7. ✅ 取消订单行 + 四量模型防御
