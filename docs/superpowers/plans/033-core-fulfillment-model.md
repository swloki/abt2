# P1: 核心履约模型

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现头行状态分离、四量模型、履行计划实体，重写 confirm() 为原子性库存预留 + 履行计划生成。

**Architecture:** 订单头状态删除 `InProduction`，仅关注交付进度。订单行新增 `SalesOrderLineStatus` 枚举和 `cancelled_qty`/`version` 字段。确认时原子性硬预留 + 生成 `fulfillment_plan_lines`。幂等头状态同步函数 `recalc_header_status`。

**前置:** P0 (AcquireChannel 枚举化) 必须已完成。

**Tech Stack:** Rust / sqlx / PostgreSQL / async-trait

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 创建 | `abt-core/migrations/033_sales_order_fulfillment.sql` | DB schema 变更 |
| 修改 | `abt-core/src/sales/sales_order/model.rs` | 枚举 + 模型定义 |
| 修改 | `abt-core/src/sales/sales_order/service.rs` | 接口扩展 |
| 修改 | `abt-core/src/sales/sales_order/implt.rs` | 核心业务逻辑重写 |
| 修改 | `abt-core/src/sales/sales_order/repo.rs` | 数据访问层 |
| 修改 | `abt-core/src/sales/sales_order/mod.rs` | 导出 |

---

## Task 1: 数据库迁移

**Files:**
- 创建: `abt-core/migrations/033_sales_order_fulfillment.sql`

- [ ] **Step 1: 编写迁移 SQL**

```sql
BEGIN;

-- =====================================================
-- 1. sales_order_items: 新增 cancelled_qty / line_status / version
-- =====================================================

ALTER TABLE sales_order_items
  ADD COLUMN IF NOT EXISTS cancelled_qty DECIMAL(18,6) NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS line_status   SMALLINT     NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS version       INT          NOT NULL DEFAULT 1;

-- line_status: 1=Pending, 2=Allocated, 3=Producing, 4=Purchasing, 5=Shipped, 6=Cancelled

-- CHECK: open_qty >= 0 (quantity - shipped_qty - cancelled_qty >= 0)
ALTER TABLE sales_order_items
  ADD CONSTRAINT chk_soi_open_qty_nonneg
  CHECK (quantity - shipped_qty - cancelled_qty >= 0);

-- =====================================================
-- 2. 状态机: 重建 SalesOrderStatus 转换矩阵（删除 InProduction）
-- =====================================================

DELETE FROM state_transition_defs
WHERE entity_type = 'SalesOrderStatus';

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('SalesOrderStatus', '',          'Draft',            NULL, 1),
    ('SalesOrderStatus', 'Draft',     'Confirmed',        NULL, 2),
    ('SalesOrderStatus', 'Confirmed', 'PartiallyShipped', NULL, 3),
    ('SalesOrderStatus', 'Confirmed', 'Shipped',          NULL, 4),
    ('SalesOrderStatus', 'PartiallyShipped', 'Shipped',   NULL, 5),
    ('SalesOrderStatus', 'Shipped',   'Completed',        NULL, 6),
    ('SalesOrderStatus', 'Draft',     'Cancelled',        NULL, 7),
    ('SalesOrderStatus', 'Confirmed', 'Cancelled',        NULL, 8),
    ('SalesOrderStatus', 'PartiallyShipped', 'Cancelled', NULL, 9)
ON CONFLICT DO NOTHING;

-- 安全兜底：将可能残留的 InProduction(3) 行修正为 Confirmed(2)
UPDATE sales_orders
SET status = 2
WHERE status = 3 AND deleted_at IS NULL;

-- =====================================================
-- 3. fulfillment_plan_lines 表
-- =====================================================

CREATE TABLE fulfillment_plan_lines (
    id                  BIGSERIAL   PRIMARY KEY,
    order_id            BIGINT      NOT NULL REFERENCES sales_orders(id),
    order_line_id       BIGINT      NOT NULL REFERENCES sales_order_items(id),
    product_id          BIGINT      NOT NULL,
    acquire_channel     SMALLINT    NOT NULL,
    required_qty        DECIMAL(18,6) NOT NULL,
    reserved_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    shortage_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    status              SMALLINT    NOT NULL DEFAULT 1,
    -- status: 1=Pending, 2=Allocated, 3=Producing, 4=Purchasing, 5=Fulfilled
    source_doc_type     SMALLINT,
    source_doc_id       BIGINT,
    reservation_details JSONB,
    required_date       DATE,
    version             INT         NOT NULL DEFAULT 1,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE fulfillment_plan_lines
  ADD CONSTRAINT chk_fpl_status
  CHECK (status IN (1, 2, 3, 4, 5));

ALTER TABLE fulfillment_plan_lines
  ADD CONSTRAINT chk_fpl_acquire_channel
  CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- 每个 order_line_id 只能有一条履行计划行
CREATE UNIQUE INDEX idx_fpl_order_line_unique
  ON fulfillment_plan_lines (order_line_id);

CREATE INDEX idx_fpl_order_id
  ON fulfillment_plan_lines (order_id);

CREATE INDEX idx_fpl_product_status
  ON fulfillment_plan_lines (product_id, status)
  WHERE status IN (1, 2, 3, 4);

COMMIT;
```

- [ ] **Step 2: 提交**

```bash
git add abt-core/migrations/033_sales_order_fulfillment.sql
git commit -m "feat(sales): add fulfillment model migration — line status, cancelled_qty, fulfillment_plan_lines"
```

---

## Task 2: 模型定义 — 新增枚举 + 更新实体

**Files:**
- 修改: `abt-core/src/sales/sales_order/model.rs`

- [ ] **Step 1: 删除 `SalesOrderStatus::InProduction`**

在 `SalesOrderStatus` 枚举中删除 `InProduction = 3` 分支，保留其他值不变：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SalesOrderStatus {
    Draft = 1,
    Confirmed = 2,
    // InProduction = 3 已删除
    PartiallyShipped = 4,
    Shipped = 5,
    Completed = 6,
    Cancelled = 7,
}
```

同步更新 `from_i16` 删除 `3 => Some(Self::InProduction)`，更新 `as_str` 删除 `InProduction` 分支。

- [ ] **Step 2: 新增 `SalesOrderLineStatus` 枚举**

在 `SalesOrderStatus` 的 serde impl 之后添加：

```rust
/// 销售订单行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SalesOrderLineStatus {
    Pending = 1,
    Allocated = 2,
    Producing = 3,
    Purchasing = 4,
    Shipped = 5,
    Cancelled = 6,
}
```

完整样板代码（`from_i16`, `as_i16`, `as_str`, `sqlx::Type`, `sqlx::Encode`, `sqlx::Decode`, `Serialize`, `Deserialize`）— 与 `SalesOrderStatus` 完全相同的模式。

- [ ] **Step 3: 新增 `FulfillmentLineStatus` 枚举**

```rust
/// 履行计划行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum FulfillmentLineStatus {
    Pending = 1,
    Allocated = 2,
    Producing = 3,
    Purchasing = 4,
    Fulfilled = 5,
}
```

同样需要完整样板代码。

- [ ] **Step 4: 更新 `SalesOrderItem` 结构体**

在 `shipped_qty` 和 `returned_qty` 之间添加三个新字段：

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesOrderItem {
    pub id: i64,
    pub order_id: i64,
    pub line_no: i32,
    pub product_id: i64,
    pub description: String,
    pub quantity: Decimal,
    pub unit: String,
    pub unit_price: Decimal,
    pub unit_cost: Decimal,
    pub discount_rate: Decimal,
    pub amount: Decimal,
    pub shipped_qty: Decimal,
    pub cancelled_qty: Decimal,              // 新增：已取消量
    pub returned_qty: Decimal,
    pub line_status: SalesOrderLineStatus,   // 新增：行级状态
    pub version: i32,                        // 新增：乐观锁
    pub delivery_date: Option<NaiveDate>,
}
```

在 `SalesOrderItem` impl 块中添加计算方法：

```rust
impl SalesOrderItem {
    /// 未交量 = ordered_qty - shipped_qty - cancelled_qty
    pub fn open_qty(&self) -> Decimal {
        self.quantity - self.shipped_qty - self.cancelled_qty
    }

    /// 是否已结清
    pub fn is_settled(&self) -> bool {
        self.shipped_qty + self.cancelled_qty >= self.quantity
    }
}
```

- [ ] **Step 5: 新增 `FulfillmentPlanLine` 实体**

```rust
/// 履行计划行实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FulfillmentPlanLine {
    pub id: i64,
    pub order_id: i64,
    pub order_line_id: i64,
    pub product_id: i64,
    pub acquire_channel: crate::master_data::product::model::AcquireChannel,
    pub required_qty: Decimal,
    pub reserved_qty: Decimal,
    pub shortage_qty: Decimal,
    pub status: FulfillmentLineStatus,
    pub source_doc_type: Option<i16>,
    pub source_doc_id: Option<i64>,
    pub reservation_details: Option<serde_json::Value>,
    pub required_date: Option<NaiveDate>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 6: 新增请求/查询类型**

```rust
/// 取消订单行请求
#[derive(Debug, Clone)]
pub struct CancelLineReq {
    pub cancelled_qty: Decimal,
}

/// 履行计划查询
#[derive(Debug, Clone, Default)]
pub struct FulfillmentPlanQuery {
    pub order_id: Option<i64>,
    pub status: Option<FulfillmentLineStatus>,
}

/// 履行计划行插入输入
pub struct FulfillmentPlanLineInput {
    pub order_id: i64,
    pub order_line_id: i64,
    pub product_id: i64,
    pub acquire_channel: crate::master_data::product::model::AcquireChannel,
    pub required_qty: Decimal,
    pub reserved_qty: Decimal,
    pub shortage_qty: Decimal,
    pub status: FulfillmentLineStatus,
    pub required_date: Option<NaiveDate>,
}
```

- [ ] **Step 7: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译错误（service/implt/repo 还没更新），记录错误

- [ ] **Step 8: 提交**

```bash
git add abt-core/src/sales/sales_order/model.rs
git commit -m "feat(sales): add line status enums, fulfillment model, four-quantity model"
```

---

## Task 3: Service 接口扩展

**Files:**
- 修改: `abt-core/src/sales/sales_order/service.rs`

- [ ] **Step 1: 更新 `SalesOrderService` trait**

删除 `start_progress` 方法，新增 4 个方法：

```rust
#[async_trait]
pub trait SalesOrderService: Send + Sync {
    // -- 现有方法（不变） --
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateSalesOrderReq) -> Result<i64>;
    async fn create_from_quotation(&self, ctx: &ServiceContext, db: PgExecutor<'_>, quotation_id: i64) -> Result<i64>;
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<SalesOrder>;
    async fn update_header(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateSalesOrderReq) -> Result<()>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateSalesOrderReq, items: Vec<CreateSalesOrderItemReq>) -> Result<()>;
    async fn list_items(&self, ctx: &ServiceContext, db: PgExecutor<'_>, order_id: i64) -> Result<Vec<SalesOrderItem>>;
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;  // 重写实现
    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;  // 更新校验
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;    // 更新实现
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, filter: SalesOrderQuery, page: PageParams) -> Result<PaginatedResult<SalesOrder>>;

    // -- 已删除 --
    // async fn start_progress(...)

    // -- 新增 P1 --
    /// 取消订单行（部分或全部）。增加 cancelled_qty。
    async fn cancel_line(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
        line_id: i64,
        req: CancelLineReq,
    ) -> Result<()>;

    /// 查询履行计划行
    async fn list_fulfillment_plan(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: FulfillmentPlanQuery,
    ) -> Result<Vec<FulfillmentPlanLine>>;

    /// 幂等重算订单头状态（根据行状态聚合推导）
    async fn recalc_header_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<SalesOrderStatus>;

    /// 手动对账：检测 fulfillment_plan_lines 与 demands 状态不一致并修复
    async fn reconcile_fulfillment_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<u32>;
}
```

- [ ] **Step 2: 新增 `ReplenishmentAllocationStrategy` trait（接口定义，P5 实现 FIFO）**

```rust
/// 分配策略接口 — P1 定义接口，P5 实现 FIFO
pub struct AllocationResult {
    pub fulfillment_line_id: i64,
    pub allocated_qty: Decimal,
}

#[async_trait]
pub trait ReplenishmentAllocationStrategy: Send + Sync {
    /// 给定可用量和候选履行计划行，按策略分配
    fn allocate(
        &self,
        product_id: i64,
        available_qty: Decimal,
        candidates: &[FulfillmentPlanLine],
    ) -> Vec<AllocationResult>;
}
```

注意：虽然这是纯计算不需要 async，但项目约定所有 service trait 使用 `#[async_trait]`。这里 trait 方法是同步的（没有 `async fn`），`#[async_trait]` 标注在 trait 上不影响同步方法。

- [ ] **Step 3: 提交**

```bash
git add abt-core/src/sales/sales_order/service.rs
git commit -m "feat(sales): extend SalesOrderService trait — cancel_line, recalc, fulfillment queries"
```

---

## Task 4: Repo 层扩展

**Files:**
- 修改: `abt-core/src/sales/sales_order/repo.rs`

- [ ] **Step 1: 更新 `ITEM_COLUMNS` 常量**

```rust
const ITEM_COLUMNS: &str = "id, order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, cancelled_qty, returned_qty, line_status, version, delivery_date";
```

注意字段顺序必须与 `SalesOrderItem` struct 字段顺序一致。

- [ ] **Step 2: 更新 SalesOrderItemRepo 的 INSERT 语句**

在 `create_batch`（或类似的批量插入方法）中添加 `cancelled_qty`, `line_status`, `version` 列：

```sql
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, cancelled_qty, line_status, version, delivery_date)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, 1, 1, $11)
```

默认值：`cancelled_qty = 0`, `line_status = 1 (Pending)`, `version = 1`。

- [ ] **Step 3: 新增 `FulfillmentPlanLineRepo`**

在 `repo.rs` 末尾添加：

```rust
// ---------------------------------------------------------------------------
// FulfillmentPlanLineRepo
// ---------------------------------------------------------------------------

pub struct FulfillmentPlanLineRepo;

impl FulfillmentPlanLineRepo {
    /// 批量插入履行计划行
    pub async fn create_batch(
        executor: PgExecutor<'_>,
        lines: &[FulfillmentPlanLineInput],
    ) -> Result<Vec<i64>> {
        let mut ids = Vec::with_capacity(lines.len());
        for line in lines {
            let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
                r#"INSERT INTO fulfillment_plan_lines
                   (order_id, order_line_id, product_id, acquire_channel, required_qty, reserved_qty, shortage_qty, status, required_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   RETURNING id"#,
            )
            .bind(line.order_id)
            .bind(line.order_line_id)
            .bind(line.product_id)
            .bind(line.acquire_channel.as_i16())
            .bind(line.required_qty)
            .bind(line.reserved_qty)
            .bind(line.shortage_qty)
            .bind(line.status.as_i16())
            .bind(line.required_date)
            .fetch_one(executor)
            .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// 按订单ID查询履行计划行
    pub async fn find_by_order_id(
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<FulfillmentPlanLine>> {
        let lines = sqlx::query_as::<sqlx::Postgres, FulfillmentPlanLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT id, order_id, order_line_id, product_id, acquire_channel, required_qty, reserved_qty, shortage_qty, status, source_doc_type, source_doc_id, reservation_details, required_date, version, created_at, updated_at FROM fulfillment_plan_lines WHERE order_id = $1"
            )),
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;
        Ok(lines)
    }

    /// 按订单行ID查询（唯一）
    pub async fn find_by_order_line_id(
        executor: PgExecutor<'_>,
        order_line_id: i64,
    ) -> Result<Option<FulfillmentPlanLine>> {
        let line = sqlx::query_as::<sqlx::Postgres, FulfillmentPlanLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT id, order_id, order_line_id, product_id, acquire_channel, required_qty, reserved_qty, shortage_qty, status, source_doc_type, source_doc_id, reservation_details, required_date, version, created_at, updated_at FROM fulfillment_plan_lines WHERE order_line_id = $1"
            )),
        )
        .bind(order_line_id)
        .fetch_optional(executor)
        .await?;
        Ok(line)
    }

    /// 更新状态（乐观锁）
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: FulfillmentLineStatus,
        expected_version: i32,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"UPDATE fulfillment_plan_lines
               SET status = $1, version = version + 1, updated_at = NOW()
               WHERE id = $2 AND version = $3"#,
        )
        .bind(status.as_i16())
        .bind(id)
        .bind(expected_version)
        .execute(executor)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(DomainError::ConcurrentConflict);
        }
        Ok(())
    }

    /// 更新下游单据关联
    pub async fn update_source_doc(
        executor: PgExecutor<'_>,
        id: i64,
        source_doc_type: i16,
        source_doc_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE fulfillment_plan_lines
               SET source_doc_type = $1, source_doc_id = $2, updated_at = NOW()
               WHERE id = $3"#,
        )
        .bind(source_doc_type)
        .bind(source_doc_id)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 4: 新增 `SalesOrderItemRepo` 的更新方法**

在现有 `SalesOrderItemRepo` impl 中添加：

```rust
    /// 批量更新行状态
    pub async fn batch_update_line_status(
        &self,
        executor: PgExecutor<'_>,
        updates: &[(i64, SalesOrderLineStatus, i32)],  // (id, new_status, expected_version)
    ) -> Result<()> {
        for (id, status, expected_version) in updates {
            let rows = sqlx::query(
                r#"UPDATE sales_order_items
                   SET line_status = $1, version = version + 1
                   WHERE id = $2 AND version = $3"#,
            )
            .bind(status.as_i16())
            .bind(id)
            .bind(expected_version)
            .execute(executor)
            .await?;

            if rows.rows_affected() == 0 {
                return Err(DomainError::ConcurrentConflict);
            }
        }
        Ok(())
    }

    /// 取消订单行（增加 cancelled_qty）
    pub async fn cancel_line(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        add_cancelled_qty: Decimal,
        new_line_status: SalesOrderLineStatus,
        expected_version: i32,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"UPDATE sales_order_items
               SET cancelled_qty = cancelled_qty + $1,
                   line_status = $2,
                   version = version + 1
               WHERE id = $3 AND version = $4
                 AND quantity - shipped_qty - cancelled_qty - $1 >= 0"#,
        )
        .bind(add_cancelled_qty)
        .bind(new_line_status.as_i16())
        .bind(id)
        .bind(expected_version)
        .execute(executor)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(DomainError::ConcurrentConflict);
        }
        Ok(())
    }
```

- [ ] **Step 5: 验证编译**

运行: `cargo clippy -p abt-core`
预期: repo 层编译通过，implt 层因缺少新方法而报错

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/sales/sales_order/repo.rs
git commit -m "feat(sales): add fulfillment plan repo and order item line status updates"
```

---

## Task 5: 核心逻辑 — implt 重写

**Files:**
- 修改: `abt-core/src/sales/sales_order/implt.rs`

这是最复杂的 Task，包含多个关键方法。

- [ ] **Step 1: 更新 struct 和 imports**

更新 `SalesOrderServiceImpl` struct：

```rust
pub struct SalesOrderServiceImpl {
    repo: SalesOrderRepo,
    item_repo: SalesOrderItemRepo,
    fp_repo: FulfillmentPlanLineRepo,  // 新增
    pool: PgPool,
}

impl SalesOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: SalesOrderRepo,
            item_repo: SalesOrderItemRepo,
            fp_repo: FulfillmentPlanLineRepo,  // 新增
            pool,
        }
    }
    // ... calculate_amounts, build_item_inputs 不变 ...
}
```

新增 imports：

```rust
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::master_data::product::model::AcquireChannel;
use super::model::*;
use super::repo::FulfillmentPlanLineRepo;
```

- [ ] **Step 2: 添加纯函数 `calc_header_status`**

在 impl 块之外添加模块级函数：

```rust
/// 幂等的订单头状态计算 — 每次订单行变更后调用
/// 关键：cancelled_qty 不等于 shipped_qty，取消不是发货
fn calc_header_status(items: &[SalesOrderItem]) -> SalesOrderStatus {
    let all_settled = items.iter().all(|i| i.is_settled());
    let any_shipped = items.iter().any(|i| i.shipped_qty > Decimal::ZERO);
    let any_open = items.iter().any(|i| i.open_qty() > Decimal::ZERO);

    if all_settled && any_shipped {
        SalesOrderStatus::Shipped
    } else if any_shipped && any_open {
        SalesOrderStatus::PartiallyShipped
    } else {
        SalesOrderStatus::Confirmed
    }
}
```

- [ ] **Step 3: 重写 `confirm()`**

核心流程变化：Draft → Confirmed 时执行原子性硬预留 + 履行计划生成。

```rust
async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
    // 1. 加载并校验
    let existing = self.repo.find_by_id(db, id).await?
        .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

    if existing.status != SalesOrderStatus::Draft {
        return Err(DomainError::business_rule("Only Draft orders can be confirmed"));
    }

    let items = self.item_repo.find_by_order_id(db, id).await?;
    if items.is_empty() {
        return Err(DomainError::business_rule("Cannot confirm order without items"));
    }

    // 2. 批量查询产品获取 acquire_channel
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let products = new_product_service(self.pool.clone())
        .get_by_ids(ctx, db, product_ids).await?;
    let product_map: std::collections::HashMap<i64, AcquireChannel> = products
        .into_iter()
        .map(|p| (p.product_id, p.acquire_channel))
        .collect();

    // 3. 状态机转换
    new_state_machine_service(self.pool.clone())
        .transition(ctx, db, "SalesOrderStatus", id, "Confirmed", None)
        .await?;
    self.repo.update_status(db, id, SalesOrderStatus::Confirmed).await?;

    // 4. 逐行处理：预留 + 生成履行计划
    let mut fp_inputs: Vec<FulfillmentPlanLineInput> = Vec::with_capacity(items.len());
    let mut line_status_updates: Vec<(i64, SalesOrderLineStatus, i32)> = Vec::with_capacity(items.len());
    let mut reserve_requests: Vec<ReserveRequest> = Vec::new();

    for item in &items {
        let ac = product_map.get(&item.product_id)
            .copied()
            .unwrap_or(AcquireChannel::Legacy);

        match ac {
            AcquireChannel::NonInventory => {
                // 费用/服务类：跳过库存，直接 Allocated
                fp_inputs.push(FulfillmentPlanLineInput {
                    order_id: id,
                    order_line_id: item.id,
                    product_id: item.product_id,
                    acquire_channel: ac,
                    required_qty: item.quantity,
                    reserved_qty: item.quantity,
                    shortage_qty: Decimal::ZERO,
                    status: FulfillmentLineStatus::Allocated,
                    required_date: item.delivery_date,
                });
                line_status_updates.push((item.id, SalesOrderLineStatus::Allocated, 1));
            }
            _ => {
                // 库存类：查询 ATP，尝试硬预留
                let total_reserved = new_inventory_reservation_service(self.pool.clone())
                    .total_reserved(ctx, db, item.product_id, None)
                    .await
                    .unwrap_or(Decimal::ZERO);

                // P1 简化策略：尝试全部硬预留，失败则为 Pending
                // 后续可引入 ATP 查询（库存总量 - 已预留量）做更精确的部分预留
                reserve_requests.push(ReserveRequest {
                    product_id: item.product_id,
                    warehouse_id: 1,  // 默认仓库
                    reserved_qty: item.quantity,
                    reservation_type: ReservationType::Hard,  // 硬预留
                    source_type: DocumentType::SalesOrder,
                    source_id: id,
                    source_line_id: Some(item.id),
                    priority: 5,
                    expires_at: None,  // 硬预留不过期
                });
            }
        }
    }

    // 5. 执行预留
    savepoint(db, "sp_reserve").await.ok();
    let mut succeeded_reservations: std::collections::HashSet<i64> = std::collections::HashSet::new();
    match new_inventory_reservation_service(self.pool.clone())
        .reserve(ctx, db, reserve_requests.clone())
        .await
    {
        Ok(batch) => {
            // 记录成功的预留行
            for req in &reserve_requests {
                if batch.failed_items.iter().all(|f| f.product_id != req.product_id) {
                    if let Some(line_id) = req.source_line_id {
                        succeeded_reservations.insert(line_id);
                    }
                }
            }
            release_savepoint(db, "sp_reserve").await.ok();
        }
        Err(e) => {
            tracing::warn!("inventory reserve error: {e}");
            rollback_savepoint(db, "sp_reserve").await.ok();
        }
    }

    // 6. 为库存类行生成履行计划（根据预留结果决定状态）
    for item in &items {
        let ac = product_map.get(&item.product_id)
            .copied()
            .unwrap_or(AcquireChannel::Legacy);
        if ac == AcquireChannel::NonInventory {
            continue;  // 已处理
        }

        let fully_reserved = succeeded_reservations.contains(&item.id);
        let (fp_status, line_status, reserved_qty, shortage_qty) = if fully_reserved {
            (FulfillmentLineStatus::Allocated, SalesOrderLineStatus::Allocated, item.quantity, Decimal::ZERO)
        } else {
            (FulfillmentLineStatus::Pending, SalesOrderLineStatus::Pending, Decimal::ZERO, item.quantity)
        };

        fp_inputs.push(FulfillmentPlanLineInput {
            order_id: id,
            order_line_id: item.id,
            product_id: item.product_id,
            acquire_channel: ac,
            required_qty: item.quantity,
            reserved_qty,
            shortage_qty,
            status: fp_status,
            required_date: item.delivery_date,
        });
        line_status_updates.push((item.id, line_status, 1));
    }

    // 7. 批量写入
    if !fp_inputs.is_empty() {
        FulfillmentPlanLineRepo::create_batch(db, &fp_inputs).await?;
    }
    if !line_status_updates.is_empty() {
        self.item_repo.batch_update_line_status(db, &line_status_updates).await?;
    }

    // 8. 审计日志
    savepoint(db, "sp_audit").await.ok();
    if let Err(e) = new_audit_log_service(self.pool.clone())
        .record(ctx, db, RecordAuditLogReq {
            entity_type: "SalesOrder",
            entity_id: id,
            action: AuditAction::Transition,
            changes: Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
            context: None,
        })
        .await
    {
        tracing::warn!("audit record failed: {e}");
        rollback_savepoint(db, "sp_audit").await.ok();
    } else {
        release_savepoint(db, "sp_audit").await.ok();
    }

    // 9. 领域事件
    savepoint(db, "sp_event").await.ok();
    if let Err(e) = new_domain_event_bus(self.pool.clone())
        .publish(ctx, db, EventPublishRequest {
            event_type: DomainEventType::SalesOrderConfirmed,
            aggregate_type: "SalesOrder".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "sales_order_id": id }),
            idempotency_key: None,
        })
        .await
    {
        tracing::warn!("event publish failed: {e}");
        rollback_savepoint(db, "sp_event").await.ok();
    } else {
        release_savepoint(db, "sp_event").await.ok();
    }

    Ok(())
}
```

- [ ] **Step 4: 删除 `start_progress()` 实现**

从 `impl SalesOrderService for SalesOrderServiceImpl` 中删除 `start_progress` 方法。`InProduction` 状态已不存在。

- [ ] **Step 5: 实现 `cancel_line()`**

```rust
async fn cancel_line(
    &self,
    ctx: &ServiceContext, db: PgExecutor<'_>,
    order_id: i64,
    line_id: i64,
    req: CancelLineReq,
) -> Result<()> {
    // 1. 校验订单状态
    let order = self.repo.find_by_id(db, order_id).await?
        .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

    if order.status != SalesOrderStatus::Confirmed
        && order.status != SalesOrderStatus::PartiallyShipped
    {
        return Err(DomainError::business_rule(
            "Only Confirmed or PartiallyShipped orders can cancel lines"
        ));
    }

    // 2. 校验订单行
    let items = self.item_repo.find_by_order_id(db, order_id).await?;
    let item = items.iter().find(|i| i.id == line_id)
        .ok_or_else(|| DomainError::not_found("SalesOrderItem"))?;

    if item.line_status == SalesOrderLineStatus::Shipped {
        return Err(DomainError::business_rule("Cannot cancel a shipped line"));
    }
    if item.line_status == SalesOrderLineStatus::Cancelled {
        return Err(DomainError::business_rule("Line is already cancelled"));
    }
    if req.cancelled_qty > item.open_qty() {
        return Err(DomainError::business_rule(
            &format!("Cancelled qty {} exceeds open qty {}", req.cancelled_qty, item.open_qty())
        ));
    }

    // 3. 更新 cancelled_qty
    let new_line_status = if item.open_qty() - req.cancelled_qty <= Decimal::ZERO {
        SalesOrderLineStatus::Cancelled
    } else {
        item.line_status
    };

    self.item_repo.cancel_line(
        db, line_id, req.cancelled_qty, new_line_status, item.version,
    ).await?;

    // 4. 如果有预留，取消对应预留量
    savepoint(db, "sp_cancel_resv").await.ok();
    if let Err(e) = new_inventory_reservation_service(self.pool.clone())
        .cancel_by_source_line(
            ctx, db,
            DocumentType::SalesOrder,
            line_id,
        )
        .await
    {
        tracing::warn!("cancel reservation for line {line_id} failed: {e}");
        rollback_savepoint(db, "sp_cancel_resv").await.ok();
    } else {
        release_savepoint(db, "sp_cancel_resv").await.ok();
    }

    // 5. 同步头状态
    self.recalc_header_status(ctx, db, order_id).await?;

    // 6. 审计
    savepoint(db, "sp_audit").await.ok();
    if let Err(e) = new_audit_log_service(self.pool.clone())
        .record(ctx, db, RecordAuditLogReq {
            entity_type: "SalesOrderItem",
            entity_id: line_id,
            action: AuditAction::Update,
            changes: Some(serde_json::json!({
                "action": "cancel_line",
                "cancelled_qty": req.cancelled_qty.to_string()
            })),
            context: None,
        })
        .await
    {
        tracing::warn!("audit record failed: {e}");
        rollback_savepoint(db, "sp_audit").await.ok();
    } else {
        release_savepoint(db, "sp_audit").await.ok();
    }

    Ok(())
}
```

- [ ] **Step 6: 实现 `recalc_header_status()`**

```rust
async fn recalc_header_status(
    &self,
    _ctx: &ServiceContext, db: PgExecutor<'_>,
    order_id: i64,
) -> Result<SalesOrderStatus> {
    let items = self.item_repo.find_by_order_id(db, order_id).await?;
    let new_status = calc_header_status(&items);

    // 仅当状态变化时才更新
    let order = self.repo.find_by_id(db, order_id).await?
        .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

    if order.status != new_status {
        // 状态机验证转换合法性
        new_state_machine_service(self.pool.clone())
            .transition(
                &ServiceContext::system(), db,
                "SalesOrderStatus", order_id,
                new_status.as_str(), None,
            )
            .await?;

        self.repo.update_status(db, order_id, new_status).await?;
    }

    Ok(new_status)
}
```

- [ ] **Step 7: 实现 `list_fulfillment_plan()`**

```rust
async fn list_fulfillment_plan(
    &self,
    _ctx: &ServiceContext, db: PgExecutor<'_>,
    query: FulfillmentPlanQuery,
) -> Result<Vec<FulfillmentPlanLine>> {
    if let Some(order_id) = query.order_id {
        FulfillmentPlanLineRepo::find_by_order_id(db, order_id).await
    } else {
        // 如果没有指定 order_id，返回空（或实现更复杂的查询）
        Ok(Vec::new())
    }
}
```

- [ ] **Step 8: 实现 `reconcile_fulfillment_status()`**

```rust
async fn reconcile_fulfillment_status(
    &self,
    _ctx: &ServiceContext, db: PgExecutor<'_>,
    order_id: i64,
) -> Result<u32> {
    // P1 简化实现：查询订单的履行计划行，检查是否有状态异常
    // 完整实现对账在 P2（demands 表就绪后）
    let _lines = FulfillmentPlanLineRepo::find_by_order_id(db, order_id).await?;
    // P2 会实现：JOIN demands 表检查不一致
    Ok(0)
}
```

- [ ] **Step 9: 更新 `complete()` 校验**

修改 `complete()` 中的数量校验，使用四量模型：

```rust
// 原来：
// if item.shipped_qty < item.quantity
// 改为：
if item.open_qty() > Decimal::ZERO
```

- [ ] **Step 10: 更新 `cancel()` 实现**

确保 `cancel()` 支持 `Confirmed` 和 `PartiallyShipped` 状态的取消，并释放所有预留：

```rust
async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
    let existing = self.repo.find_by_id(db, id).await?
        .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

    if existing.status != SalesOrderStatus::Draft
        && existing.status != SalesOrderStatus::Confirmed
        && existing.status != SalesOrderStatus::PartiallyShipped
    {
        return Err(DomainError::business_rule(
            "Only Draft, Confirmed or PartiallyShipped orders can be cancelled"
        ));
    }

    // 状态机转换
    new_state_machine_service(self.pool.clone())
        .transition(ctx, db, "SalesOrderStatus", id, "Cancelled", None)
        .await?;
    self.repo.update_status(db, id, SalesOrderStatus::Cancelled).await?;

    // 释放所有预留
    savepoint(db, "sp_cancel_resv").await.ok();
    if let Err(e) = new_inventory_reservation_service(self.pool.clone())
        .cancel_by_source(ctx, db, DocumentType::SalesOrder, id)
        .await
    {
        tracing::warn!("cancel reservations failed: {e}");
        rollback_savepoint(db, "sp_cancel_resv").await.ok();
    } else {
        release_savepoint(db, "sp_cancel_resv").await.ok();
    }

    // 审计 + 事件（与现有模式相同）
    // ...

    Ok(())
}
```

- [ ] **Step 11: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译通过（可能有小错误需要修复）

- [ ] **Step 12: 提交**

```bash
git add abt-core/src/sales/sales_order/implt.rs
git commit -m "feat(sales): rewrite confirm with atomic reservation + fulfillment plan, add cancel_line and recalc"
```

---

## Task 6: 更新 mod.rs 导出

**Files:**
- 修改: `abt-core/src/sales/sales_order/mod.rs`

- [ ] **Step 1: 更新导出**

```rust
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::SalesOrderService;

use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    implt::SalesOrderServiceImpl::new(pool)
}
```

- [ ] **Step 2: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译通过

- [ ] **Step 3: 提交**

```bash
git add abt-core/src/sales/sales_order/mod.rs
git commit -m "feat(sales): update module exports for fulfillment model"
```

---

## Task 7: P1 最终验证

- [ ] **Step 1: 全量编译检查**

运行: `cargo clippy -p abt-core`
预期: 零错误零警告

- [ ] **Step 2: 运行现有测试**

运行: `cargo test -p abt-core`
预期: 所有现有测试通过

- [ ] **Step 3: 检查 abt-web 编译**

运行: `cargo clippy -p abt-web`
预期: 可能因为 `start_progress` 方法被删除而报错。这是预期的——abt-web 的调整在 P3 阶段处理。如果报错，记录错误内容以便后续修复。

- [ ] **Step 4: 最终提交（如有修复）**

```bash
git add -A
git commit -m "fix(sales): address clippy warnings from P1 changes"
```
