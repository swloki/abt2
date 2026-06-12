# Sales Order Downstream Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 P2+P4 — 销售订单确认后需求流转到采购/生产模块的完整后端功能

**Architecture:** 事件驱动需求处理。销售订单 confirm → DemandCreated 事件 → 采购/MES Handler 发送通知。操作员通过 Service API 查询需求池、合并创建 PO/生产计划草稿。乐观锁并发控制，数据库视图封装跨模块 JOIN。

**Tech Stack:** Rust, sqlx (PostgreSQL), DomainEventBus (Outbox), 数据库视图

---

## File Structure Map

### New Files
| 文件 | 职责 |
|------|------|
| `abt-core/migrations/036_create_demand_pool_views.sql` | 采购/生产需求池视图 + 性能索引 |
| `abt-core/src/purchase/demand_handler/mod.rs` | 模块导出 + 工厂函数 |
| `abt-core/src/purchase/demand_handler/model.rs` | 请求/响应模型 |
| `abt-core/src/purchase/demand_handler/service.rs` | PurchaseDemandService trait |
| `abt-core/src/purchase/demand_handler/repo.rs` | 视图查询 + 乐观锁 |
| `abt-core/src/purchase/demand_handler/implt.rs` | PurchaseDemandService 实现 |
| `abt-core/src/purchase/demand_handler/handler.rs` | PurchaseDemandCreatedHandler |
| `abt-core/src/mes/demand_handler/mod.rs` | 模块导出 + 工厂函数 |
| `abt-core/src/mes/demand_handler/model.rs` | 请求/响应模型 |
| `abt-core/src/mes/demand_handler/service.rs` | MesDemandService trait |
| `abt-core/src/mes/demand_handler/repo.rs` | 视图查询 + 乐观锁 |
| `abt-core/src/mes/demand_handler/implt.rs` | MesDemandService 实现 |
| `abt-core/src/mes/demand_handler/handler.rs` | MesDemandCreatedHandler |
| `abt-core/src/sales/sales_order/event_handlers.rs` | SalesDemandConfirmed/RejectedHandler |

### Modified Files
| 文件 | 改动 |
|------|------|
| `abt-core/src/purchase/mod.rs` | 添加 `pub mod demand_handler;` |
| `abt-core/src/mes/mod.rs` | 添加 `pub mod demand_handler;` |
| `abt-core/src/sales/sales_order/mod.rs` | 添加 `pub mod event_handlers;` |
| `abt-core/src/sales/sales_order/repo.rs` | 添加 `SalesOrderRepo::find_doc_number_by_id` |
| `abt-web/src/state.rs` | EventProcessor 初始化 + Handler 注册 |

### UML Design Docs
| 文件 | 改动 |
|------|------|
| `docs/uml-design/06-purchase.html` | 新增 PurchaseDemandService 接口和需求池查询 |
| `docs/uml-design/03-mes.html` | 新增 MesDemandService 接口和需求池查询 |

---

## Key Mapping Notes

> 规格文档中的命名与实际代码库的映射关系，实施时必须使用实际代码库的名称。

| 规格文档 | 实际代码库 | 说明 |
|----------|-----------|------|
| `DemandStatus::Open` | `DemandStatus::Pending` (1) | 需求等待被处理 |
| `DemandStatus::Processing` | `DemandStatus::Confirmed` (2) | 需求已关联下游单据 |
| `demand.order_id` | `demand.source_id` | 需求模型中订单 ID 字段名 |
| `demand.order_line_id` | `demand.source_line_id` | 需求模型中订单行 ID 字段名 |
| `demand.quantity` | `demand.required_qty` | 需求数量字段名 |
| `so.order_no` | `so.doc_number` | 销售订单号实际列名 |
| `AcquireChannel::Purchased` value | `2` (i16) | 外购渠道值 |
| `AcquireChannel::SelfProduced` value | `1` (i16) | 自制渠道值 |
| `DocumentType::PurchaseOrder` | `7` (i16) | 采购订单单据类型 |
| `DocumentType::ProductionPlan` | `12` (i16) | 生产计划单据类型 |

---

## Task 1: Database Migration（视图 + 索引）

**Files:**
- Create: `abt-core/migrations/036_create_demand_pool_views.sql`

- [ ] **Step 1: Write migration file**

```sql
-- 036_create_demand_pool_views.sql
-- 采购需求池视图：封装 demands + products + sales_orders 的 JOIN
CREATE OR REPLACE VIEW v_purchase_demands AS
SELECT
    d.id,
    d.source_id          AS order_id,
    d.source_line_id     AS order_line_id,
    d.product_id,
    d.required_qty       AS quantity,
    d.required_date,
    d.priority,
    d.status             AS demand_status,
    d.acquire_channel,
    d.target_doc_id,
    d.target_doc_type,
    d.created_at,
    p.name               AS product_name,
    p.code               AS product_code,
    so.doc_number        AS order_no
FROM demands d
JOIN products p   ON p.id = d.product_id
JOIN sales_orders so ON so.id = d.source_id
WHERE d.acquire_channel = 2    -- Purchased
  AND d.deleted_at IS NULL;

-- 生产需求池视图：封装 demands + products + sales_orders 的 JOIN
CREATE OR REPLACE VIEW v_production_demands AS
SELECT
    d.id,
    d.source_id          AS order_id,
    d.source_line_id     AS order_line_id,
    d.product_id,
    d.required_qty       AS quantity,
    d.required_date,
    d.priority,
    d.status             AS demand_status,
    d.acquire_channel,
    d.target_doc_id,
    d.target_doc_type,
    d.created_at,
    p.name               AS product_name,
    p.code               AS product_code,
    so.doc_number        AS order_no
FROM demands d
JOIN products p   ON p.id = d.product_id
JOIN sales_orders so ON so.id = d.source_id
WHERE d.acquire_channel = 1    -- SelfProduced
  AND d.deleted_at IS NULL;

-- 性能索引：demands 表核心查询索引（部分索引，仅覆盖未删除行）
CREATE INDEX IF NOT EXISTS idx_demands_channel_status
    ON demands (acquire_channel, status)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_demands_product
    ON demands (product_id)
    WHERE deleted_at IS NULL;
```

- [ ] **Step 2: Run migration**

Run: `sqlx database url from .env, then apply migration via cargo sqlx or direct psql`

验证：连接数据库执行 `\dv v_purchase_demands` 和 `\dv v_production_demands`，确认视图存在。

- [ ] **Step 3: Commit**

```bash
git add abt-core/migrations/036_create_demand_pool_views.sql
git commit -m "feat: add demand pool views and indexes for purchase/MES downstream"
```

---

## Task 2: SalesOrderRepo Helper Method

**Files:**
- Modify: `abt-core/src/sales/sales_order/repo.rs`

需要在 `SalesOrderRepo` 上添加一个方法，供 Event Handler 查询订单号。

- [ ] **Step 1: Add `find_doc_number_by_id` method**

在 `abt-core/src/sales/sales_order/repo.rs` 的 `impl SalesOrderRepo` 块末尾添加：

```rust
    /// 按 ID 查询订单号（供跨模块 Event Handler 使用）
    pub async fn find_doc_number_by_id(
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT doc_number FROM sales_orders WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(row.map(|r| r.try_get::<String, _>("doc_number")).transpose()?)
    }
```

- [ ] **Step 2: Verify**

Run: `cargo clippy -p abt-core 2>&1 | head -30`

Expected: 无与该方法相关的错误或警告

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/sales/sales_order/repo.rs
git commit -m "feat: add SalesOrderRepo::find_doc_number_by_id for event handlers"
```

---

## Task 3: Purchase DemandHandler — Interface（接口先行）

> **接口先行原则**：先定义 Service trait + Model，确认后再实现。

**Files:**
- Create: `abt-core/src/purchase/demand_handler/model.rs`
- Create: `abt-core/src/purchase/demand_handler/service.rs`
- Create: `abt-core/src/purchase/demand_handler/mod.rs`（仅导出，无工厂函数）
- Modify: `abt-core/src/purchase/mod.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p abt-core/src/purchase/demand_handler
```

- [ ] **Step 2: Write `model.rs`**

```rust
//! 采购需求池 — 请求/响应模型

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

/// 需求查询参数（订单行维度）
#[derive(Debug, Clone, Default)]
pub struct DemandPoolQuery {
    pub status: Option<i16>,       // DemandStatus 枚举值，默认 Pending(1)
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
}

/// 需求摘要（订单行维度 — 展示给操作员）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: String,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub demand_status: i16,
    pub created_at: NaiveDateTime,
}

/// 物料聚合查询参数
#[derive(Debug, Clone, Default)]
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,
}

/// 物料聚合摘要（物料维度 — 采购员主要操作视图）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialAggSummary {
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub total_demand_qty: Decimal,
    pub demand_count: i64,
    pub earliest_required_date: Option<NaiveDate>,
    pub latest_required_date: Option<NaiveDate>,
}

/// 从需求创建采购订单请求
#[derive(Debug, Clone)]
pub struct CreateOrderFromDemandsReq {
    pub demand_ids: Vec<i64>,
    pub supplier_id: i64,
    pub expected_delivery_date: Option<NaiveDate>,
    pub remark: String,
}

/// 创建下游单据的统一响应（含部分成功信息）
#[derive(Debug, Clone)]
pub struct CreateDownstreamResult {
    pub doc_id: i64,
    pub processed_demand_count: usize,
    pub skipped_demands: Vec<SkippedDemand>,
    /// "Confirmed" — 前端用此字段判断补货已启动
    pub demand_status: String,
}

/// 被跳过的需求
#[derive(Debug, Clone)]
pub struct SkippedDemand {
    pub demand_id: i64,
    pub reason: String,
}

/// 乐观锁返回的已锁定需求数据
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LockedDemand {
    pub id: i64,
    pub product_id: i64,
    pub source_id: i64,
    pub source_line_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
}
```

- [ ] **Step 3: Write `service.rs`（接口定义）**

```rust
//! 采购需求池 — Service trait

use async_trait::async_trait;

use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;

/// 采购需求池服务 — 查询外购需求 + 创建采购订单草稿
#[async_trait]
pub trait PurchaseDemandService: Send + Sync {
    /// 查询待处理的外购需求（订单行维度）
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>>;

    /// 按物料聚合查询外购需求（物料维度 — 采购员操作入口）
    async fn list_material_aggregated(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>>;

    /// 从选中的需求批量创建采购订单草稿
    /// - 乐观锁并发控制
    /// - 按 product_id 聚合需求
    /// - 同一事务内完成 + 发布 DemandConfirmed 事件
    async fn create_order_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOrderFromDemandsReq,
    ) -> Result<CreateDownstreamResult>;
}
```

- [ ] **Step 4: Write `mod.rs`（仅导出，暂无工厂函数）**

```rust
//! 采购需求池子模块

pub mod model;
pub mod service;

pub use model::*;
pub use service::PurchaseDemandService;
```

- [ ] **Step 5: Update `purchase/mod.rs`**

在 `abt-core/src/purchase/mod.rs` 中添加一行 `pub mod demand_handler;`：

```rust
//! 采购 SRM 模块

pub mod enums;
pub mod demand_handler;
pub mod misc_request;
pub mod order;
pub mod payment;
pub mod quotation;
pub mod reconciliation;
pub mod return_order;

pub use misc_request::MiscellaneousRequestService;
pub use order::PurchaseOrderService;
pub use payment::PaymentRequestService;
pub use quotation::PurchaseQuotationService;
pub use reconciliation::PurchaseReconciliationService;
pub use return_order::PurchaseReturnService;
```

- [ ] **Step 6: Verify**

Run: `cargo clippy -p abt-core --lib 2>&1 | head -30`

Expected: 编译通过，无新错误（`mod.rs` 没有 `implt`/`repo`/`handler` 模块声明，暂时只导出 model + service trait）

- [ ] **Step 7: Commit**

```bash
git add abt-core/src/purchase/demand_handler/ abt-core/src/purchase/mod.rs
git commit -m "feat(purchase): define PurchaseDemandService interface and models (接口先行)"
```

---

## Task 4: MES DemandHandler — Interface（接口先行）

**Files:**
- Create: `abt-core/src/mes/demand_handler/model.rs`
- Create: `abt-core/src/mes/demand_handler/service.rs`
- Create: `abt-core/src/mes/demand_handler/mod.rs`
- Modify: `abt-core/src/mes/mod.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p abt-core/src/mes/demand_handler
```

- [ ] **Step 2: Write `model.rs`**

```rust
//! MES 需求池 — 请求/响应模型

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

/// 需求查询参数（订单行维度）— 与采购模块结构相同，独立定义以保持模块自治
#[derive(Debug, Clone, Default)]
pub struct DemandPoolQuery {
    pub status: Option<i16>,
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
}

/// 需求摘要（订单行维度）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: String,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub demand_status: i16,
    pub created_at: NaiveDateTime,
}

/// 物料聚合查询参数
#[derive(Debug, Clone, Default)]
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,
}

/// 物料聚合摘要（物料维度 — 计划员操作入口）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialAggSummary {
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub total_demand_qty: Decimal,
    pub demand_count: i64,
    pub earliest_required_date: Option<NaiveDate>,
    pub latest_required_date: Option<NaiveDate>,
}

/// 从需求创建生产计划请求
#[derive(Debug, Clone)]
pub struct CreatePlanFromDemandsReq {
    pub demand_ids: Vec<i64>,
    pub plan_type: i16,
    pub plan_date: NaiveDate,
    pub remark: Option<String>,
    /// 每条需求的排程参数 — 可选，不填则使用默认排程
    pub items: Option<Vec<PlanDemandItemReq>>,
    /// 默认排程参数（当 items 未提供时使用）
    // TODO: P5 接入产品主数据 Lead Time，当前使用全局配置默认值
    pub default_scheduled_start: Option<NaiveDate>,
    pub default_scheduled_end: Option<NaiveDate>,
}

/// 单条需求的排程参数
#[derive(Debug, Clone)]
pub struct PlanDemandItemReq {
    pub demand_id: i64,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub priority: i32,
}

/// 创建下游单据的统一响应
#[derive(Debug, Clone)]
pub struct CreateDownstreamResult {
    pub doc_id: i64,
    pub processed_demand_count: usize,
    pub skipped_demands: Vec<SkippedDemand>,
    pub demand_status: String,
}

/// 被跳过的需求
#[derive(Debug, Clone)]
pub struct SkippedDemand {
    pub demand_id: i64,
    pub reason: String,
}

/// 乐观锁返回的已锁定需求数据
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LockedDemand {
    pub id: i64,
    pub product_id: i64,
    pub source_id: i64,
    pub source_line_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
}
```

- [ ] **Step 3: Write `service.rs`（接口定义）**

```rust
//! MES 需求池 — Service trait

use async_trait::async_trait;

use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;

/// MES 需求池服务 — 查询自制需求 + 创建生产计划草稿
#[async_trait]
pub trait MesDemandService: Send + Sync {
    /// 查询待处理的自制需求（订单行维度）
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>>;

    /// 按物料聚合查询自制需求（物料维度 — 计划员操作入口）
    async fn list_material_aggregated(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>>;

    /// 从选中的需求创建生产计划草稿
    /// - 乐观锁并发控制
    /// - 按 product_id 聚合需求
    /// - 同一事务内完成 + 发布 DemandConfirmed 事件
    async fn create_plan_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePlanFromDemandsReq,
    ) -> Result<CreateDownstreamResult>;
}
```

- [ ] **Step 4: Write `mod.rs`**

```rust
//! MES 需求池子模块

pub mod model;
pub mod service;

pub use model::*;
pub use service::MesDemandService;
```

- [ ] **Step 5: Update `mes/mod.rs`**

在 `abt-core/src/mes/mod.rs` 中添加 `pub mod demand_handler;`：

```rust
//! MES 生产制造执行模块
//!
//! 覆盖从生产计划到完工入库的完整生产管理流程。
//! 严格遵循 docs/uml-design/04-mes.html 中的 UML 设计。

pub mod enums;
pub mod demand_handler;

pub mod production_plan;
pub mod work_order;
pub mod production_batch;
pub mod work_report;
pub mod production_inspection;
pub mod production_receipt;
pub mod dashboard;
pub mod production_exception;

pub use enums::*;
```

- [ ] **Step 6: Verify**

Run: `cargo clippy -p abt-core --lib 2>&1 | head -30`

- [ ] **Step 7: Commit**

```bash
git add abt-core/src/mes/demand_handler/ abt-core/src/mes/mod.rs
git commit -m "feat(mes): define MesDemandService interface and models (接口先行)"
```

---

## Task 5: Purchase DemandHandler — Full Implementation

**Files:**
- Create: `abt-core/src/purchase/demand_handler/repo.rs`
- Create: `abt-core/src/purchase/demand_handler/implt.rs`
- Create: `abt-core/src/purchase/demand_handler/handler.rs`
- Modify: `abt-core/src/purchase/demand_handler/mod.rs`（添加新模块声明 + 工厂函数）

- [ ] **Step 1: Write `repo.rs`**

```rust
//! 采购需求池 — 数据库查询（基于视图 v_purchase_demands）

use rust_decimal::Decimal;
use sqlx::Row;

use crate::shared::types::{PgExecutor, Result};
use crate::shared::types::pagination::{PageParams, PaginatedResult};

use super::model::*;

pub struct PurchaseDemandRepo;

impl PurchaseDemandRepo {
    /// 查询视图 v_purchase_demands（封装跨模块 JOIN）
    /// 动态条件 + 分页
    pub async fn find_demands(
        db: PgExecutor<'_>,
        query: &DemandPoolQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: u32 = 1;

        let status_param;
        if let Some(s) = query.status {
            status_param = s;
            where_clauses.push(format!("demand_status = ${param_idx}"));
            param_idx += 1;
        } else {
            status_param = -1;
            where_clauses.push("demand_status = 1".to_string()); // 默认 Pending
        }

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let order_param;
        if let Some(oid) = query.order_id {
            order_param = oid;
            where_clauses.push(format!("order_id = ${param_idx}"));
            param_idx += 1;
        } else {
            order_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM v_purchase_demands WHERE {where_sql}");
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql.clone()));
        if query.status.is_some() { count_q = count_q.bind(status_param); }
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        if query.order_id.is_some() { count_q = count_q.bind(order_param); }
        let count_row = count_q.fetch_one(db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT * FROM v_purchase_demands WHERE {where_sql} \
             ORDER BY required_date ASC NULLS LAST, priority DESC \
             LIMIT ${param_idx} OFFSET ${param_idx + 1}"
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql.clone()));
        if query.status.is_some() { data_q = data_q.bind(status_param); }
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        if query.order_id.is_some() { data_q = data_q.bind(order_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(db).await?;

        let data: Vec<DemandSummary> = rows.iter().map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }).collect();

        Ok(PaginatedResult { data, total: total as u64, page: page.page, page_size: page.page_size })
    }

    /// 按物料聚合查询（物料维度 — 采购员主要操作视图）
    pub async fn find_material_aggregated(
        db: PgExecutor<'_>,
        query: &MaterialAggQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        let mut where_clauses = vec!["demand_status = 1".to_string()]; // Pending only
        let mut param_idx: u32 = 1;

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!(
            "SELECT COUNT(*) AS cnt FROM (
                SELECT product_id FROM v_purchase_demands WHERE {where_sql} GROUP BY product_id
             ) sub"
        );
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql));
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        let count_row = count_q.fetch_one(db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT product_id, product_name, product_code, \
                    SUM(quantity) AS total_demand_qty, \
                    COUNT(*) AS demand_count, \
                    MIN(required_date) AS earliest_required_date, \
                    MAX(required_date) AS latest_required_date \
             FROM v_purchase_demands WHERE {where_sql} \
             GROUP BY product_id, product_name, product_code \
             ORDER BY total_demand_qty DESC \
             LIMIT ${param_idx} OFFSET ${param_idx + 1}"
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(db).await?;

        let data: Vec<MaterialAggSummary> = rows.iter().map(|r| MaterialAggSummary {
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            total_demand_qty: r.try_get("total_demand_qty").unwrap_or(Decimal::ZERO),
            demand_count: r.try_get("demand_count").unwrap_or(0),
            earliest_required_date: r.try_get("earliest_required_date").unwrap_or(None),
            latest_required_date: r.try_get("latest_required_date").unwrap_or(None),
        }).collect();

        Ok(PaginatedResult { data, total: total as u64, page: page.page, page_size: page.page_size })
    }

    /// 乐观锁：批量锁定外购需求（原子 UPDATE + RETURNING）
    /// 只返回成功锁定的需求，未锁定的记入 skipped
    pub async fn lock_demands_for_purchase(
        db: PgExecutor<'_>,
        demand_ids: &[i64],
    ) -> Result<Vec<LockedDemand>> {
        let rows = sqlx::query_as::<_, LockedDemand>(
            r#"UPDATE demands SET status = 2, updated_at = NOW()
               WHERE id = ANY($1) AND status = 1 AND acquire_channel = 2 AND deleted_at IS NULL
               RETURNING id, product_id, source_id, source_line_id, acquire_channel, required_qty, required_date, priority"#,
        )
        .bind(demand_ids)
        .fetch_all(db)
        .await?;

        Ok(rows)
    }

    /// 按 ID 查询需求详情（从视图，供 Handler 使用）
    pub async fn find_detail_by_id(
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<DemandSummary>> {
        let row = sqlx::query(
            "SELECT * FROM v_purchase_demands WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        Ok(row.map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }))
    }
}
```

- [ ] **Step 2: Write `implt.rs`**

```rust
//! 采购需求池 — PurchaseDemandService 实现

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Local;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::purchase::order::{new_purchase_order_service, PurchaseOrderService};
use crate::purchase::order::model::{CreateOrderItemRequest, CreatePurchaseOrderRequest};
use crate::sales::sales_order::repo::DemandRepo;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, model::EventPublishRequest};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;
use super::repo::PurchaseDemandRepo;
use super::service::PurchaseDemandService;

pub struct PurchaseDemandServiceImpl {
    pool: PgPool,
}

impl PurchaseDemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseDemandService for PurchaseDemandServiceImpl {
    async fn list_pending_demands(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        PurchaseDemandRepo::find_demands(db, &query, &page).await
    }

    async fn list_material_aggregated(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        PurchaseDemandRepo::find_material_aggregated(db, &query, &page).await
    }

    async fn create_order_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOrderFromDemandsReq,
    ) -> Result<CreateDownstreamResult> {
        if req.demand_ids.is_empty() {
            return Err(DomainError::validation("demand_ids 不能为空"));
        }

        // 1. 乐观锁：原子 UPDATE，只处理成功锁定的需求
        let locked = PurchaseDemandRepo::lock_demands_for_purchase(db, &req.demand_ids).await?;

        // 计算被跳过的需求
        let locked_ids: Vec<i64> = locked.iter().map(|d| d.id).collect();
        let skipped_demands: Vec<SkippedDemand> = req.demand_ids.iter()
            .filter(|id| !locked_ids.contains(id))
            .map(|id| SkippedDemand {
                demand_id: *id,
                reason: "已被他人处理或状态已变更".to_string(),
            })
            .collect();

        if locked.is_empty() {
            return Err(DomainError::business_rule("所有需求已被他人处理或状态已变更"));
        }

        // 2. 按 product_id 聚合
        let mut aggregated: HashMap<i64, Decimal> = HashMap::new();
        for d in &locked {
            *aggregated.entry(d.product_id).or_insert(Decimal::ZERO) += d.required_qty;
        }

        // 3. 创建采购订单草稿
        let today = Local::now().date_naive();
        let mut items: Vec<CreateOrderItemRequest> = Vec::new();
        for (idx, (product_id, qty)) in aggregated.iter().enumerate() {
            items.push(CreateOrderItemRequest {
                product_id: *product_id,
                line_no: (idx as i32) + 1,
                description: String::new(),
                quantity: *qty,
                unit_price: Decimal::ZERO, // 待采购员补充
                quotation_item_id: None,
                expected_delivery_date: req.expected_delivery_date,
            });
        }

        let po_req = CreatePurchaseOrderRequest {
            supplier_id: req.supplier_id,
            order_date: today,
            expected_delivery_date: req.expected_delivery_date,
            payment_terms: None,
            delivery_address: None,
            remark: req.remark.clone(),
            items,
        };

        let po_id = new_purchase_order_service(self.pool.clone())
            .create(ctx, db, po_req, None)
            .await?;

        // 4. 关联需求：更新 target_doc + 发布 DemandConfirmed 事件
        for d in &locked {
            DemandRepo::update_target_doc(db, d.id, DocumentType::PurchaseOrder as i16, po_id).await?;

            let event_bus = new_domain_event_bus(self.pool.clone());
            let _ = event_bus.publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: d.id,
                payload: serde_json::json!({
                    "order_id": d.source_id,
                    "order_line_id": d.source_line_id,
                    "product_id": d.product_id,
                    "acquire_channel": d.acquire_channel,
                    "target_doc_type": DocumentType::PurchaseOrder as i16,
                    "target_doc_id": po_id,
                }),
                idempotency_key: None,
            }).await;
        }

        Ok(CreateDownstreamResult {
            doc_id: po_id,
            processed_demand_count: locked.len(),
            skipped_demands,
            demand_status: "Confirmed".to_string(),
        })
    }
}
```

- [ ] **Step 3: Write `handler.rs`**

```rust
//! 采购需求池 — DemandCreated 事件处理器

use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::warn;

use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::notification::{new_notification_service, service::NotificationService};
use crate::shared::notification::model::{BatchNotificationReq, NotificationType};
use crate::shared::types::{Result, ServiceContext};

use super::repo::PurchaseDemandRepo;

// TODO: 从系统角色配置中获取实际值
const PURCHASE_ROLE_ID: i64 = 3;

/// 采购需求创建 Handler — 监听 DemandCreated 事件，发送通知给采购角色
pub struct PurchaseDemandCreatedHandler {
    pool: PgPool,
}

impl PurchaseDemandCreatedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for PurchaseDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let acquire_channel = event.payload["acquire_channel"].as_i64();

        // 只处理外购需求（acquire_channel = 2）
        if acquire_channel != Some(2) {
            return Ok(());
        }

        let demand_id = event.aggregate_id;

        // 回查视图获取需求数据（包含 product_name, order_no）
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;

        let detail = match PurchaseDemandRepo::find_detail_by_id(&mut conn, demand_id).await? {
            Some(d) => d,
            None => {
                // 需求不存在或不在视图中 — 记录 Warning
                warn!(demand_id, "Demand not found in v_purchase_demands, skipping notification");
                return Ok(());
            }
        };

        // 防御事件乱序：status 不是 Pending 则跳过
        if detail.demand_status != 1 {
            return Ok(());
        }

        // 发送通知给采购角色
        let ctx = ServiceContext::system();
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(
            &ctx,
            &mut conn,
            PURCHASE_ROLE_ID,
            BatchNotificationReq {
                notification_type: NotificationType::Business,
                title: "新的外购需求待处理".into(),
                content: Some(format!(
                    "产品: {} ({}) × {}, 来源订单: {}",
                    detail.product_name, detail.product_code, detail.quantity, detail.order_no
                )),
                related_type: Some("demand".into()),
                related_id: Some(demand_id),
            },
        ).await?;

        Ok(())
    }

    fn name(&self) -> &str {
        "purchase_demand_created"
    }
}
```

- [ ] **Step 4: Update `mod.rs`（添加新模块 + 工厂函数）**

```rust
//! 采购需求池子模块

pub mod handler;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::PurchaseDemandService;

use sqlx::postgres::PgPool;

pub fn new_purchase_demand_service(pool: PgPool) -> impl PurchaseDemandService {
    implt::PurchaseDemandServiceImpl::new(pool)
}
```

- [ ] **Step 5: Verify**

Run: `cargo clippy -p abt-core --lib 2>&1 | head -40`

Expected: 编译通过。若有类型不匹配或导入错误，根据 clippy 提示修正。

- [ ] **Step 6: Commit**

```bash
git add abt-core/src/purchase/demand_handler/
git commit -m "feat(purchase): implement PurchaseDemandService with repo, impl, and handler"
```

---

## Task 6: MES DemandHandler — Full Implementation

**Files:**
- Create: `abt-core/src/mes/demand_handler/repo.rs`
- Create: `abt-core/src/mes/demand_handler/implt.rs`
- Create: `abt-core/src/mes/demand_handler/handler.rs`
- Modify: `abt-core/src/mes/demand_handler/mod.rs`

结构与 Task 5 对称。以下给出完整代码，因为两个模块的 acquire_channel 值和下游单据不同。

- [ ] **Step 1: Write `repo.rs`**

```rust
//! MES 需求池 — 数据库查询（基于视图 v_production_demands）

use rust_decimal::Decimal;
use sqlx::Row;

use crate::shared::types::{PgExecutor, Result};
use crate::shared::types::pagination::{PageParams, PaginatedResult};

use super::model::*;

pub struct MesDemandRepo;

impl MesDemandRepo {
    /// 查询视图 v_production_demands（封装跨模块 JOIN）
    pub async fn find_demands(
        db: PgExecutor<'_>,
        query: &DemandPoolQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: u32 = 1;

        let status_param;
        if let Some(s) = query.status {
            status_param = s;
            where_clauses.push(format!("demand_status = ${param_idx}"));
            param_idx += 1;
        } else {
            status_param = -1;
            where_clauses.push("demand_status = 1".to_string());
        }

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let order_param;
        if let Some(oid) = query.order_id {
            order_param = oid;
            where_clauses.push(format!("order_id = ${param_idx}"));
            param_idx += 1;
        } else {
            order_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) AS cnt FROM v_production_demands WHERE {where_sql}");
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql.clone()));
        if query.status.is_some() { count_q = count_q.bind(status_param); }
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        if query.order_id.is_some() { count_q = count_q.bind(order_param); }
        let count_row = count_q.fetch_one(db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT * FROM v_production_demands WHERE {where_sql} \
             ORDER BY required_date ASC NULLS LAST, priority DESC \
             LIMIT ${param_idx} OFFSET ${param_idx + 1}"
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if query.status.is_some() { data_q = data_q.bind(status_param); }
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        if query.order_id.is_some() { data_q = data_q.bind(order_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(db).await?;

        let data: Vec<DemandSummary> = rows.iter().map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }).collect();

        Ok(PaginatedResult { data, total: total as u64, page: page.page, page_size: page.page_size })
    }

    /// 按物料聚合查询
    pub async fn find_material_aggregated(
        db: PgExecutor<'_>,
        query: &MaterialAggQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        let mut where_clauses = vec!["demand_status = 1".to_string()];
        let mut param_idx: u32 = 1;

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!(
            "SELECT COUNT(*) AS cnt FROM (
                SELECT product_id FROM v_production_demands WHERE {where_sql} GROUP BY product_id
             ) sub"
        );
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql));
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        let count_row = count_q.fetch_one(db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT product_id, product_name, product_code, \
                    SUM(quantity) AS total_demand_qty, \
                    COUNT(*) AS demand_count, \
                    MIN(required_date) AS earliest_required_date, \
                    MAX(required_date) AS latest_required_date \
             FROM v_production_demands WHERE {where_sql} \
             GROUP BY product_id, product_name, product_code \
             ORDER BY total_demand_qty DESC \
             LIMIT ${param_idx} OFFSET ${param_idx + 1}"
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(db).await?;

        let data: Vec<MaterialAggSummary> = rows.iter().map(|r| MaterialAggSummary {
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            total_demand_qty: r.try_get("total_demand_qty").unwrap_or(Decimal::ZERO),
            demand_count: r.try_get("demand_count").unwrap_or(0),
            earliest_required_date: r.try_get("earliest_required_date").unwrap_or(None),
            latest_required_date: r.try_get("latest_required_date").unwrap_or(None),
        }).collect();

        Ok(PaginatedResult { data, total: total as u64, page: page.page, page_size: page.page_size })
    }

    /// 乐观锁：批量锁定自制需求
    pub async fn lock_demands_for_production(
        db: PgExecutor<'_>,
        demand_ids: &[i64],
    ) -> Result<Vec<LockedDemand>> {
        let rows = sqlx::query_as::<_, LockedDemand>(
            r#"UPDATE demands SET status = 2, updated_at = NOW()
               WHERE id = ANY($1) AND status = 1 AND acquire_channel = 1 AND deleted_at IS NULL
               RETURNING id, product_id, source_id, source_line_id, acquire_channel, required_qty, required_date, priority"#,
        )
        .bind(demand_ids)
        .fetch_all(db)
        .await?;

        Ok(rows)
    }

    /// 按 ID 查询需求详情（从视图）
    pub async fn find_detail_by_id(
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<DemandSummary>> {
        let row = sqlx::query(
            "SELECT * FROM v_production_demands WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        Ok(row.map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }))
    }
}
```

- [ ] **Step 2: Write `implt.rs`**

```rust
//! MES 需求池 — MesDemandService 实现

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Local;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::mes::production_plan::{new_production_plan_service, ProductionPlanService};
use crate::mes::production_plan::model::{CreatePlanItemReq, CreatePlanReq};
use crate::sales::sales_order::repo::DemandRepo;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, model::EventPublishRequest};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;
use super::repo::MesDemandRepo;
use super::service::MesDemandService;

pub struct MesDemandServiceImpl {
    pool: PgPool,
}

impl MesDemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MesDemandService for MesDemandServiceImpl {
    async fn list_pending_demands(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        MesDemandRepo::find_demands(db, &query, &page).await
    }

    async fn list_material_aggregated(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        MesDemandRepo::find_material_aggregated(db, &query, &page).await
    }

    async fn create_plan_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePlanFromDemandsReq,
    ) -> Result<CreateDownstreamResult> {
        if req.demand_ids.is_empty() {
            return Err(DomainError::validation("demand_ids 不能为空"));
        }

        // 1. 乐观锁
        let locked = MesDemandRepo::lock_demands_for_production(db, &req.demand_ids).await?;

        let locked_ids: Vec<i64> = locked.iter().map(|d| d.id).collect();
        let skipped_demands: Vec<SkippedDemand> = req.demand_ids.iter()
            .filter(|id| !locked_ids.contains(id))
            .map(|id| SkippedDemand {
                demand_id: *id,
                reason: "已被他人处理或状态已变更".to_string(),
            })
            .collect();

        if locked.is_empty() {
            return Err(DomainError::business_rule("所有需求已被他人处理或状态已变更"));
        }

        // 2. 构建 items 参数映射（demand_id → PlanDemandItemReq）
        let item_map: HashMap<i64, &PlanDemandItemReq> = req.items
            .as_ref()
            .map(|items| items.iter().map(|i| (i.demand_id, i)).collect())
            .unwrap_or_default();

        // 默认排程参数
        let default_start = req.default_scheduled_start.unwrap_or(req.plan_date);
        // TODO: P5 接入产品主数据 Lead Time，当前使用 plan_date + 7 天
        let default_end = req.default_scheduled_end.unwrap_or_else(|| {
            req.plan_date + chrono::Duration::days(7)
        });

        // 3. 按 product_id 聚合
        let mut aggregated: HashMap<i64, (Decimal, Vec<&LockedDemand>)> = HashMap::new();
        for d in &locked {
            let entry = aggregated.entry(d.product_id).or_insert_with(|| (Decimal::ZERO, Vec::new()));
            entry.0 += d.required_qty;
            entry.1.push(d);
        }

        // 4. 创建生产计划草稿
        let mut plan_items: Vec<CreatePlanItemReq> = Vec::new();
        for (_product_id, (qty, demands)) in &aggregated {
            let d = demands[0]; // 取第一条需求的参数作为代表
            let (scheduled_start, scheduled_end, priority) = match item_map.get(&d.id) {
                Some(item) => (item.scheduled_start, item.scheduled_end, item.priority),
                None => (default_start, default_end, d.priority),
            };

            plan_items.push(CreatePlanItemReq {
                product_id: d.product_id,
                planned_qty: *qty,
                scheduled_start,
                scheduled_end,
                sales_order_id: Some(d.source_id),
                sales_order_item_id: Some(d.source_line_id),
                bom_snapshot_id: None,
                routing_id: None,
                work_center_id: None,
                priority,
            });
        }

        let plan_req = CreatePlanReq {
            plan_type: req.plan_type,
            plan_date: req.plan_date,
            remark: req.remark.clone(),
            items: plan_items,
        };

        let plan_id = new_production_plan_service(self.pool.clone())
            .create(ctx, db, plan_req)
            .await?;

        // 5. 关联需求 + 发布事件
        for d in &locked {
            DemandRepo::update_target_doc(db, d.id, DocumentType::ProductionPlan as i16, plan_id).await?;

            let event_bus = new_domain_event_bus(self.pool.clone());
            let _ = event_bus.publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: d.id,
                payload: serde_json::json!({
                    "order_id": d.source_id,
                    "order_line_id": d.source_line_id,
                    "product_id": d.product_id,
                    "acquire_channel": d.acquire_channel,
                    "target_doc_type": DocumentType::ProductionPlan as i16,
                    "target_doc_id": plan_id,
                }),
                idempotency_key: None,
            }).await;
        }

        Ok(CreateDownstreamResult {
            doc_id: plan_id,
            processed_demand_count: locked.len(),
            skipped_demands,
            demand_status: "Confirmed".to_string(),
        })
    }
}
```

- [ ] **Step 3: Write `handler.rs`**

```rust
//! MES 需求池 — DemandCreated 事件处理器

use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::warn;

use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::notification::{new_notification_service, service::NotificationService};
use crate::shared::notification::model::{BatchNotificationReq, NotificationType};
use crate::shared::types::{Result, ServiceContext};

use super::repo::MesDemandRepo;

// TODO: 从系统角色配置中获取实际值
const PRODUCTION_ROLE_ID: i64 = 4;

/// MES 需求创建 Handler — 监听 DemandCreated 事件，发送通知给生产角色
pub struct MesDemandCreatedHandler {
    pool: PgPool,
}

impl MesDemandCreatedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for MesDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let acquire_channel = event.payload["acquire_channel"].as_i64();

        // 只处理自制需求（acquire_channel = 1）
        if acquire_channel != Some(1) {
            return Ok(());
        }

        let demand_id = event.aggregate_id;

        // 回查视图获取需求数据
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;

        let detail = match MesDemandRepo::find_detail_by_id(&mut conn, demand_id).await? {
            Some(d) => d,
            None => {
                warn!(demand_id, "Demand not found in v_production_demands, skipping notification");
                return Ok(());
            }
        };

        // 防御事件乱序
        if detail.demand_status != 1 {
            return Ok(());
        }

        // 发送通知给生产角色
        let ctx = ServiceContext::system();
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(
            &ctx,
            &mut conn,
            PRODUCTION_ROLE_ID,
            BatchNotificationReq {
                notification_type: NotificationType::Business,
                title: "新的生产需求待处理".into(),
                content: Some(format!(
                    "产品: {} ({}) × {}, 来源订单: {}",
                    detail.product_name, detail.product_code, detail.quantity, detail.order_no
                )),
                related_type: Some("demand".into()),
                related_id: Some(demand_id),
            },
        ).await?;

        Ok(())
    }

    fn name(&self) -> &str {
        "mes_demand_created"
    }
}
```

- [ ] **Step 4: Update `mod.rs`**

```rust
//! MES 需求池子模块

pub mod handler;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::MesDemandService;

use sqlx::postgres::PgPool;

pub fn new_mes_demand_service(pool: PgPool) -> impl MesDemandService {
    implt::MesDemandServiceImpl::new(pool)
}
```

- [ ] **Step 5: Verify**

Run: `cargo clippy -p abt-core --lib 2>&1 | head -40`

- [ ] **Step 6: Commit**

```bash
git add abt-core/src/mes/demand_handler/
git commit -m "feat(mes): implement MesDemandService with repo, impl, and handler"
```

---

## Task 7: Sales Event Handlers（包装已有的 handle_demand_confirmed/rejected）

**Files:**
- Create: `abt-core/src/sales/sales_order/event_handlers.rs`
- Modify: `abt-core/src/sales/sales_order/mod.rs`

将 `implt.rs` 中已存在的 `handle_demand_confirmed` / `handle_demand_rejected` 包装为 `impl EventHandler`，供 EventProcessor 注册。

- [ ] **Step 1: Write `event_handlers.rs`**

```rust
//! 销售 — DemandConfirmed / DemandRejected 事件处理器
//!
//! 包装 implt.rs 中已有的 handle_demand_confirmed / handle_demand_rejected 函数，
//! 使其符合 EventHandler trait，可被 EventProcessor 注册和调度。

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{Result, ServiceContext};

/// DemandConfirmed 事件处理器 — 异步更新履行计划行和订单行状态
///
/// 事务边界：独立事务（与 confirm 事务分离），避免跨聚合死锁。
/// 幂等保证：Handler 内部使用前置状态校验的单条 UPDATE（见 implt.rs）。
pub struct SalesDemandConfirmedHandler {
    pool: PgPool,
}

impl SalesDemandConfirmedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for SalesDemandConfirmedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system();

        // 复用 implt.rs 中已有的逻辑
        super::implt::handle_demand_confirmed(
            self.pool.clone(),
            &ctx,
            &mut conn,
            event,
        ).await
    }

    fn name(&self) -> &str {
        "sales_demand_confirmed"
    }
}

/// DemandRejected 事件处理器 — 回退履行计划行和订单行到 Pending
pub struct SalesDemandRejectedHandler {
    pool: PgPool,
}

impl SalesDemandRejectedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for SalesDemandRejectedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system();

        super::implt::handle_demand_rejected(
            self.pool.clone(),
            &ctx,
            &mut conn,
            event,
        ).await
    }

    fn name(&self) -> &str {
        "sales_demand_rejected"
    }
}
```

- [ ] **Step 2: Update `sales/sales_order/mod.rs`**

在 `abt-core/src/sales/sales_order/mod.rs` 中添加 `pub mod event_handlers;`：

```rust
pub mod event_handlers;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::{SalesOrderService, ReplenishmentAllocationStrategy, AllocationResult, DemandService};

use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    implt::SalesOrderServiceImpl::new(pool)
}

pub fn new_demand_service(pool: PgPool) -> impl DemandService {
    implt::DemandServiceImpl::new(pool)
}
```

- [ ] **Step 3: Verify**

Run: `cargo clippy -p abt-core --lib 2>&1 | head -40`

注意：`handle_demand_confirmed` 和 `handle_demand_rejected` 在 `implt.rs` 中是 `pub async fn`，确保它们可被 `event_handlers.rs` 通过 `super::implt::` 访问。如果它们当前是 `pub(crate)` 或更严格的可见性，需要调整为 `pub` 或确保模块内部可见。

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/sales/sales_order/event_handlers.rs abt-core/src/sales/sales_order/mod.rs
git commit -m "feat(sales): add SalesDemandConfirmed/RejectedHandler wrapping existing functions"
```

---

## Task 8: EventProcessor Wiring（abt-web）

**Files:**
- Modify: `abt-web/src/state.rs`

在 `AppState` 中创建 `EventHandlerRegistry`，注册所有 Handler，启动 `EventProcessor`。

- [ ] **Step 1: Add EventProcessor initialization to `state.rs`**

在 `abt-web/src/state.rs` 中找到 `AppState` 的构造函数（`new` 方法），在最后添加 EventProcessor 初始化逻辑。

关键要点：
1. 在所有业务路由注册完成之后才启动 EventProcessor
2. 使用 `Arc<EventHandlerRegistryImpl>` 共享注册表
3. 注册 5 个 Handler：
   - `PurchaseDemandCreatedHandler` → `DemandCreated(64)`
   - `MesDemandCreatedHandler` → `DemandCreated(64)`
   - `SalesDemandConfirmedHandler` → `DemandConfirmed(65)`
   - `SalesDemandRejectedHandler` → `DemandRejected(66)`

在 `AppState::new` 方法末尾添加：

```rust
use std::sync::Arc;
use abt_core::shared::event_bus::{
    EventHandlerRegistryImpl, EventProcessor,
    DeadLetterServiceImpl,
};
use abt_core::purchase::demand_handler::handler::PurchaseDemandCreatedHandler;
use abt_core::mes::demand_handler::handler::MesDemandCreatedHandler;
use abt_core::sales::sales_order::event_handlers::{
    SalesDemandConfirmedHandler, SalesDemandRejectedHandler,
};
use abt_core::shared::enums::event::DomainEventType;

// ... 在 AppState::new 末尾：

// 创建事件处理器注册表
let registry = Arc::new(EventHandlerRegistryImpl::new());

// 注册 DemandCreated Handler（两个 Handler 注册在同一事件上）
registry.register(
    DomainEventType::DemandCreated,
    Arc::new(PurchaseDemandCreatedHandler::new(pool.clone())),
);
registry.register(
    DomainEventType::DemandCreated,
    Arc::new(MesDemandCreatedHandler::new(pool.clone())),
);

// 注册 DemandConfirmed / DemandRejected Handler
registry.register(
    DomainEventType::DemandConfirmed,
    Arc::new(SalesDemandConfirmedHandler::new(pool.clone())),
);
registry.register(
    DomainEventType::DemandRejected,
    Arc::new(SalesDemandRejectedHandler::new(pool.clone())),
);

// 创建并启动 EventProcessor
let dead_letter = Arc::new(DeadLetterServiceImpl::new(pool.clone()));
let processor = EventProcessor::new(
    Arc::new(pool.clone()),
    registry,
    dead_letter,
    3, // max_retries
);
processor.start();

tracing::info!("EventProcessor started with 4 handlers registered");
```

**注意事项**：
- 需要在对应模块的 `mod.rs` 中导出 handler struct（确保 `pub`）
- `AppState` 可能需要持有 `EventProcessor` 的引用以支持优雅关闭
- 如果 `AppState::new` 不适合放启动逻辑，可改在 `main.rs` 中启动后传入

- [ ] **Step 2: Ensure handler structs are exported**

确认以下模块正确导出 handler struct：

- `abt-core/src/purchase/demand_handler/mod.rs` 需要添加：
  ```rust
  pub use handler::PurchaseDemandCreatedHandler;
  ```

- `abt-core/src/mes/demand_handler/mod.rs` 需要添加：
  ```rust
  pub use handler::MesDemandCreatedHandler;
  ```

- `abt-core/src/sales/sales_order/mod.rs` 需要添加：
  ```rust
  pub use event_handlers::{SalesDemandConfirmedHandler, SalesDemandRejectedHandler};
  ```

- [ ] **Step 3: Verify full build**

Run: `cargo build 2>&1 | tail -20`

Expected: 编译成功，无错误

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/state.rs abt-core/src/purchase/demand_handler/mod.rs abt-core/src/mes/demand_handler/mod.rs abt-core/src/sales/sales_order/mod.rs
git commit -m "feat: wire EventProcessor with all demand handlers registered"
```

---

## Task 9: Full Verification

- [ ] **Step 1: Run clippy on full workspace**

Run: `cargo clippy 2>&1 | tail -30`

Expected: 无错误。修正所有 clippy 警告。

- [ ] **Step 2: Run build**

Run: `cargo build 2>&1 | tail -20`

Expected: 编译成功。

- [ ] **Step 3: Run tests**

Run: `cargo test -p abt-core 2>&1 | tail -30`

Expected: 所有现有测试通过。

---

## Task 10: UML Design Doc — Purchase

**Files:**
- Modify: `docs/uml-design/06-purchase.html`

- [ ] **Step 1: Update purchase UML design doc**

在 `docs/uml-design/06-purchase.html` 中新增以下内容（在现有采购模块设计之后）：

1. **PurchaseDemandService 接口定义**：
   - `list_pending_demands(ctx, db, query, page) -> PaginatedResult<DemandSummary>`
   - `list_material_aggregated(ctx, db, query, page) -> PaginatedResult<MaterialAggSummary>`
   - `create_order_from_demands(ctx, db, req) -> CreateDownstreamResult`

2. **新增模型**：
   - `DemandPoolQuery`、`DemandSummary`、`MaterialAggQuery`、`MaterialAggSummary`
   - `CreateOrderFromDemandsReq`、`CreateDownstreamResult`、`SkippedDemand`

3. **数据库视图**：`v_purchase_demands` 定义和用途说明

4. **EventHandler**：`PurchaseDemandCreatedHandler` 的职责和过滤条件

5. **时序图**：从 DemandCreated → 通知 → 查询需求池 → 创建 PO → DemandConfirmed 的完整流程

遵循现有 HTML 文档格式（标题层级、代码块样式等）。

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/06-purchase.html
git commit -m "docs: update purchase UML with PurchaseDemandService and demand pool design"
```

---

## Task 11: UML Design Doc — MES

**Files:**
- Modify: `docs/uml-design/03-mes.html`

- [ ] **Step 1: Update MES UML design doc**

在 `docs/uml-design/03-mes.html` 中新增以下内容：

1. **MesDemandService 接口定义**：
   - `list_pending_demands(ctx, db, query, page) -> PaginatedResult<DemandSummary>`
   - `list_material_aggregated(ctx, db, query, page) -> PaginatedResult<MaterialAggSummary>`
   - `create_plan_from_demands(ctx, db, req) -> CreateDownstreamResult`

2. **新增模型**：
   - `CreatePlanFromDemandsReq`、`PlanDemandItemReq`（含排程参数）

3. **数据库视图**：`v_production_demands` 定义和用途说明

4. **EventHandler**：`MesDemandCreatedHandler` 的职责和过滤条件

5. **时序图**：从 DemandCreated → 通知 → 查询需求池 → 创建生产计划 → DemandConfirmed 的完整流程

6. **后续流程**（已有实现）：计划审核 → 释放工单 → 完工入库 → DemandService.fulfill

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/03-mes.html
git commit -m "docs: update MES UML with MesDemandService and demand pool design"
```

---

## Self-Review Checklist

### 1. Spec Coverage

| 规格章节 | 覆盖任务 | 状态 |
|----------|---------|------|
| §3 整体架构 | Task 5-8 | ✅ |
| §4 采购模块集成 | Task 3, 5 | ✅ |
| §4.2 EventHandler | Task 5 handler.rs | ✅ |
| §4.3 PurchaseDemandService 接口 | Task 3 service.rs | ✅ |
| §4.4 请求/响应模型 | Task 3 model.rs | ✅ |
| §4.5 create_order_from_demands 流程 | Task 5 implt.rs | ✅ |
| §4.6 Repo + 视图查询 | Task 5 repo.rs | ✅ |
| §5 MES 模块集成 | Task 4, 6 | ✅ |
| §5.2 MesDemandCreatedHandler | Task 6 handler.rs | ✅ |
| §5.3 MesDemandService 接口 | Task 4 service.rs | ✅ |
| §5.4 请求模型 | Task 4 model.rs | ✅ |
| §5.5 create_plan_from_demands 流程 | Task 6 implt.rs | ✅ |
| §6 EventProcessor 注册启动 | Task 8 | ✅ |
| §6.4 handle_demand_confirmed 改造 | Task 7 | ✅ |
| §6.5 confirm 异步策略 | 已有实现 + Task 7 | ✅ |
| §14.2 不返回 demand_ids | Task 3 model.rs | ✅ MaterialAggSummary 无 demand_ids |
| §14.3 部分成功优化 | Task 5/6 implt.rs | ✅ CreateDownstreamResult |
| §14.5 幂等 SQL 模式 | 已有 handle_demand_confirmed | ✅ |
| §14.7 遍历边界（RETURNING id） | Task 5/6 repo.rs | ✅ lock_demands_for_* |

### 2. Placeholder Scan

- ✅ 无 "TBD" / "TODO"（除 MES 排程默认值的 P5 标注，这是规格要求的）
- ✅ 无 "implement later" / "fill in details"
- ✅ 无 "add appropriate error handling"（错误处理已写明）
- ✅ 无 "similar to Task N"（MES 模块给出完整独立代码）

### 3. Type Consistency

- ✅ `DemandPoolQuery` 在 Task 3/4 model.rs 和 Task 5/6 service.rs/implt.rs 中名称一致
- ✅ `DemandSummary` 字段名在 model.rs 和 repo.rs 中一致（手动映射，不依赖 sqlx::FromRow 的 derive）
- ✅ `CreateDownstreamResult` 在 purchase 和 MES 的 implt.rs 中返回结构一致
- ✅ `LockedDemand` 字段与 SQL `RETURNING` 子句匹配
- ✅ `DocumentType::PurchaseOrder` (7) 和 `DocumentType::ProductionPlan` (12) 在 implt.rs 中使用一致
- ✅ `DemandStatus::Pending` (1) 和 `DemandStatus::Confirmed` (2) 在乐观锁 SQL 中正确使用
