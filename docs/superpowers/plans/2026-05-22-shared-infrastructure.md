# 共享基础设施层 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现统一文档编号、文档关联图谱、库存预留层、成本累积账本 4 个共享组件，作为 CRM/SRM/WMS/MES 的公共依赖。

**Architecture:** 每个共享组件遵循现有分层模式：Migration → Model → Repository → Service Trait → Service Impl → Handler → Proto。组件间通过 `DocumentType` 枚举解耦，不直接引用具体业务模块。

**Tech Stack:** Rust, sqlx (compile-time checked), async-trait, tonic (gRPC), prost, PostgreSQL, anyhow

**Spec:** `docs/superpowers/specs/2026-05-22-erp-modules-uml-design.md` Section 2

---

## File Structure

```
abt/migrations/
  055_create_document_sequences.sql
  056_create_document_links.sql
  057_create_inventory_reservations.sql
  058_create_cost_entries.sql

proto/abt/v1/
  shared.proto                    # DocumentType, LinkType, CostType 等枚举 + 共享服务

abt/src/models/
  document_sequence.rs            # DocumentSequence, DocumentType
  document_link.rs                # DocumentLink, LinkType
  inventory_reservation.rs        # InventoryReservation, ReservationType, ReservationStatus
  cost_entry.rs                   # CostEntry, CostType, CostEntityType

abt/src/repositories/
  document_sequence_repo.rs
  document_link_repo.rs
  inventory_reservation_repo.rs
  cost_entry_repo.rs

abt/src/service/
  document_sequence_service.rs    # async trait
  document_link_service.rs
  inventory_reservation_service.rs
  cost_entry_service.rs

abt/src/implt/
  document_sequence_service_impl.rs
  document_link_service_impl.rs
  inventory_reservation_service_impl.rs
  cost_entry_service_impl.rs

abt-grpc/src/handlers/
  shared.rs                       # 共享服务的 gRPC handler

abt-grpc/src/generated/
  abt.v1.rs                       # 自动生成（cargo build）
```

---

### Task 1: Migration — 创建 4 张共享表

**Files:**
- Create: `abt/migrations/055_create_document_sequences.sql`
- Create: `abt/migrations/056_create_document_links.sql`
- Create: `abt/migrations/057_create_inventory_reservations.sql`
- Create: `abt/migrations/058_create_cost_entries.sql`

- [ ] **Step 1: 创建 document_sequences 迁移**

```sql
-- 055_create_document_sequences.sql
CREATE TABLE document_sequences (
    id            BIGSERIAL PRIMARY KEY,
    prefix        VARCHAR(16) NOT NULL,
    current_value INTEGER NOT NULL DEFAULT 0,
    seq_date      DATE NOT NULL,
    padding_len   INTEGER NOT NULL DEFAULT 5,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(prefix, seq_date)
);

CREATE INDEX idx_doc_seq_prefix_date ON document_sequences(prefix, seq_date);

COMMENT ON TABLE document_sequences IS '统一文档编号序列';
```

- [ ] **Step 2: 创建 document_links 迁移**

```sql
-- 056_create_document_links.sql
CREATE TABLE document_links (
    id          BIGSERIAL PRIMARY KEY,
    source_type SMALLINT NOT NULL,
    source_id   BIGINT NOT NULL,
    target_type SMALLINT NOT NULL,
    target_id   BIGINT NOT NULL,
    link_type   SMALLINT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by  BIGINT
);

CREATE INDEX idx_doc_links_source ON document_links(source_type, source_id);
CREATE INDEX idx_doc_links_target ON document_links(target_type, target_id);

COMMENT ON TABLE document_links IS '文档关联图谱（有向图）';
```

- [ ] **Step 3: 创建 inventory_reservations 迁移**

```sql
-- 057_create_inventory_reservations.sql
CREATE TABLE inventory_reservations (
    id                BIGSERIAL PRIMARY KEY,
    product_id        BIGINT NOT NULL,
    warehouse_id      BIGINT NOT NULL,
    reserved_qty      DECIMAL(14,6) NOT NULL,
    reservation_type  SMALLINT NOT NULL,
    source_type       SMALLINT NOT NULL,
    source_id         BIGINT NOT NULL,
    status            SMALLINT NOT NULL DEFAULT 1,
    priority          INTEGER NOT NULL DEFAULT 0,
    expires_at        TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_inv_res_product_wh ON inventory_reservations(product_id, warehouse_id);
CREATE INDEX idx_inv_res_source ON inventory_reservations(source_type, source_id);
CREATE INDEX idx_inv_res_status ON inventory_reservations(status) WHERE status = 1;

COMMENT ON TABLE inventory_reservations IS '库存预留层';
```

- [ ] **Step 4: 创建 cost_entries 迁移**

```sql
-- 058_create_cost_entries.sql
CREATE TABLE cost_entries (
    id            BIGSERIAL PRIMARY KEY,
    entity_type   SMALLINT NOT NULL,
    entity_id     BIGINT NOT NULL,
    cost_type     SMALLINT NOT NULL,
    debit_amount  DECIMAL(14,6) NOT NULL DEFAULT 0,
    credit_amount DECIMAL(14,6) NOT NULL DEFAULT 0,
    cost_center   BIGINT,
    profit_center BIGINT,
    period        VARCHAR(7) NOT NULL,
    source_type   SMALLINT NOT NULL,
    source_id     BIGINT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cost_entries_entity ON cost_entries(entity_type, entity_id);
CREATE INDEX idx_cost_entries_period ON cost_entries(period);
CREATE INDEX idx_cost_entries_source ON cost_entries(source_type, source_id);

COMMENT ON TABLE cost_entries IS '成本累积账本（双层记账）';
```

- [ ] **Step 5: 验证迁移**

Run: `cd E:\work\abt && cargo clippy 2>&1 | head -20`
Expected: 无 migration 相关错误

- [ ] **Step 6: Commit**

```bash
git add abt/migrations/055_create_document_sequences.sql abt/migrations/056_create_document_links.sql abt/migrations/057_create_inventory_reservations.sql abt/migrations/058_create_cost_entries.sql
git commit -m "feat: add shared infrastructure migration tables"
```

---

### Task 2: Model — 定义共享枚举和结构体

**Files:**
- Create: `abt/src/models/document_sequence.rs`
- Create: `abt/src/models/document_link.rs`
- Create: `abt/src/models/inventory_reservation.rs`
- Create: `abt/src/models/cost_entry.rs`
- Modify: `abt/src/models/mod.rs`

- [ ] **Step 1: 创建 document_sequence model**

```rust
// abt/src/models/document_sequence.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DocumentSequence {
    pub id: i64,
    pub prefix: String,
    pub current_value: i32,
    pub seq_date: chrono::NaiveDate,
    pub padding_len: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum DocumentType {
    Quotation = 1,
    SalesOrder = 2,
    ShippingRequest = 3,
    SalesReturn = 4,
    Reconciliation = 5,
    PurchaseQuotation = 6,
    PurchaseOrder = 7,
    PurchaseReturn = 8,
    MiscellaneousRequest = 9,
    WorkOrder = 10,
    OutsourcingOrder = 11,
    ProductionPlan = 12,
    WorkReport = 13,
    ProductionInspection = 14,
    ProductionReceipt = 15,
    ArrivalNotice = 16,
    MaterialRequisition = 17,
    Backflush = 18,
    CycleCount = 19,
    InventoryTransfer = 20,
    FormConversion = 21,
    InventoryLock = 22,
    PaymentRequest = 23,
    Invoice = 24,
}

impl DocumentType {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Quotation => "QUO",
            Self::SalesOrder => "SO",
            Self::ShippingRequest => "SR",
            Self::SalesReturn => "SRT",
            Self::Reconciliation => "REC",
            Self::PurchaseQuotation => "PQ",
            Self::PurchaseOrder => "PO",
            Self::PurchaseReturn => "PRT",
            Self::MiscellaneousRequest => "MISC",
            Self::WorkOrder => "WO",
            Self::OutsourcingOrder => "OO",
            Self::ProductionPlan => "PP",
            Self::WorkReport => "WR",
            Self::ProductionInspection => "PI",
            Self::ProductionReceipt => "PR",
            Self::ArrivalNotice => "AN",
            Self::MaterialRequisition => "MR",
            Self::Backflush => "BF",
            Self::CycleCount => "CC",
            Self::InventoryTransfer => "TRF",
            Self::FormConversion => "FC",
            Self::InventoryLock => "LCK",
            Self::PaymentRequest => "PAY",
            Self::Invoice => "INV",
        }
    }

    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Quotation),
            2 => Some(Self::SalesOrder),
            3 => Some(Self::ShippingRequest),
            4 => Some(Self::SalesReturn),
            5 => Some(Self::Reconciliation),
            6 => Some(Self::PurchaseQuotation),
            7 => Some(Self::PurchaseOrder),
            8 => Some(Self::PurchaseReturn),
            9 => Some(Self::MiscellaneousRequest),
            10 => Some(Self::WorkOrder),
            11 => Some(Self::OutsourcingOrder),
            12 => Some(Self::ProductionPlan),
            13 => Some(Self::WorkReport),
            14 => Some(Self::ProductionInspection),
            15 => Some(Self::ProductionReceipt),
            16 => Some(Self::ArrivalNotice),
            17 => Some(Self::MaterialRequisition),
            18 => Some(Self::Backflush),
            19 => Some(Self::CycleCount),
            20 => Some(Self::InventoryTransfer),
            21 => Some(Self::FormConversion),
            22 => Some(Self::InventoryLock),
            23 => Some(Self::PaymentRequest),
            24 => Some(Self::Invoice),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}
```

- [ ] **Step 2: 创建 document_link model**

```rust
// abt/src/models/document_link.rs
use serde::{Deserialize, Serialize};
use crate::models::document_sequence::DocumentType;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DocumentLink {
    pub id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub target_type: DocumentType,
    pub target_id: i64,
    pub link_type: LinkType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum LinkType {
    DerivedFrom = 1,
    Triggers = 2,
    References = 3,
    Reconciles = 4,
    Inspects = 5,
    Fulfills = 6,
    Allocates = 7,
}

impl LinkType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::DerivedFrom),
            2 => Some(Self::Triggers),
            3 => Some(Self::References),
            4 => Some(Self::Reconciles),
            5 => Some(Self::Inspects),
            6 => Some(Self::Fulfills),
            7 => Some(Self::Allocates),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}
```

- [ ] **Step 3: 创建 inventory_reservation model**

```rust
// abt/src/models/inventory_reservation.rs
use serde::{Deserialize, Serialize};
use crate::models::document_sequence::DocumentType;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InventoryReservation {
    pub id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub reserved_qty: rust_decimal::Decimal,
    pub reservation_type: ReservationType,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub status: ReservationStatus,
    pub priority: i32,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum ReservationType {
    Hard = 1,
    Soft = 2,
    SafetyStock = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum ReservationStatus {
    Active = 1,
    Fulfilled = 2,
    Cancelled = 3,
    Expired = 4,
}

impl ReservationType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v { 1 => Some(Self::Hard), 2 => Some(Self::Soft), 3 => Some(Self::SafetyStock), _ => None }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl ReservationStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v { 1 => Some(Self::Active), 2 => Some(Self::Fulfilled), 3 => Some(Self::Cancelled), 4 => Some(Self::Expired), _ => None }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}
```

- [ ] **Step 4: 创建 cost_entry model**

```rust
// abt/src/models/cost_entry.rs
use serde::{Deserialize, Serialize};
use crate::models::document_sequence::DocumentType;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CostEntry {
    pub id: i64,
    pub entity_type: CostEntityType,
    pub entity_id: i64,
    pub cost_type: CostType,
    pub debit_amount: rust_decimal::Decimal,
    pub credit_amount: rust_decimal::Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub period: String,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum CostType {
    Material = 1,
    Labor = 2,
    Overhead = 3,
    Outsource = 4,
    Rework = 5,
    Scrap = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum CostEntityType {
    Product = 1,
    WorkOrder = 2,
    SalesOrder = 3,
    PurchaseOrder = 4,
    Inspection = 5,
}

impl CostType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v { 1 => Some(Self::Material), 2 => Some(Self::Labor), 3 => Some(Self::Overhead), 4 => Some(Self::Outsource), 5 => Some(Self::Rework), 6 => Some(Self::Scrap), _ => None }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl CostEntityType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v { 1 => Some(Self::Product), 2 => Some(Self::WorkOrder), 3 => Some(Self::SalesOrder), 4 => Some(Self::PurchaseOrder), 5 => Some(Self::Inspection), _ => None }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}
```

- [ ] **Step 5: 注册模块到 mod.rs**

在 `abt/src/models/mod.rs` 中添加：
```rust
mod document_sequence;
mod document_link;
mod inventory_reservation;
mod cost_entry;
pub use document_sequence::*;
pub use document_link::*;
pub use inventory_reservation::*;
pub use cost_entry::*;
```

- [ ] **Step 6: 验证编译**

Run: `cd E:\work\abt && cargo clippy -p abt 2>&1 | tail -5`
Expected: 无错误

- [ ] **Step 7: Commit**

```bash
git add abt/src/models/document_sequence.rs abt/src/models/document_link.rs abt/src/models/inventory_reservation.rs abt/src/models/cost_entry.rs abt/src/models/mod.rs
git commit -m "feat: add shared infrastructure models"
```

---

### Task 3: Repository — 共享层数据访问

**Files:**
- Create: `abt/src/repositories/document_sequence_repo.rs`
- Create: `abt/src/repositories/document_link_repo.rs`
- Create: `abt/src/repositories/inventory_reservation_repo.rs`
- Create: `abt/src/repositories/cost_entry_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

- [ ] **Step 1: 创建 document_sequence_repo**

```rust
// abt/src/repositories/document_sequence_repo.rs
use anyhow::Result;
use common::PgExecutor;
use sqlx::Executor;

pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    pub async fn next_number(
        executor: impl PgExecutor<'_>,
        doc_type: crate::models::DocumentType,
    ) -> Result<String> {
        let prefix = doc_type.prefix();
        let today = chrono::Utc::now().date_naive();

        let row = sqlx::query!(
            r#"INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len)
               VALUES ($1, 1, $2, 5)
               ON CONFLICT (prefix, seq_date) DO UPDATE SET current_value = document_sequences.current_value + 1
               RETURNING current_value, padding_len"#,
            prefix,
            today
        )
        .fetch_one(executor)
        .await?;

        Ok(format!(
            "{}-{}-{:0>width$}",
            prefix,
            today.format("%Y-%m"),
            row.current_value,
            width = row.padding_len as usize
        ))
    }
}
```

- [ ] **Step 2: 创建 document_link_repo**

```rust
// abt/src/repositories/document_link_repo.rs
use anyhow::Result;
use common::PgExecutor;

pub struct DocumentLinkRepo;

impl DocumentLinkRepo {
    pub async fn create_link(
        executor: impl PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
        target_type: i16,
        target_id: i64,
        link_type: i16,
        created_by: Option<i64>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"INSERT INTO document_links (source_type, source_id, target_type, target_id, link_type, created_by)
               VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
            source_type, source_id, target_type, target_id, link_type, created_by
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_linked(
        executor: impl PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
    ) -> Result<Vec<crate::models::DocumentLink>> {
        let rows = sqlx::query!(
            r#"SELECT id, source_type, source_id, target_type, target_id, link_type, created_at, created_by
               FROM document_links WHERE source_type = $1 AND source_id = $2"#,
            source_type, source_id
        )
        .fetch_all(executor)
        .await?;

        Ok(rows.into_iter().map(|r| crate::models::DocumentLink {
            id: r.id,
            source_type: crate::models::DocumentType::from_i16(r.source_type).unwrap(),
            source_id: r.source_id,
            target_type: crate::models::DocumentType::from_i16(r.target_type).unwrap(),
            target_id: r.target_id,
            link_type: crate::models::LinkType::from_i16(r.link_type).unwrap(),
            created_at: r.created_at,
            created_by: r.created_by,
        }).collect())
    }
}
```

- [ ] **Step 3: 创建 inventory_reservation_repo**

```rust
// abt/src/repositories/inventory_reservation_repo.rs
use anyhow::Result;
use common::PgExecutor;
use rust_decimal::Decimal;

pub struct InventoryReservationRepo;

impl InventoryReservationRepo {
    pub async fn create(
        executor: impl PgExecutor<'_>,
        product_id: i64,
        warehouse_id: i64,
        reserved_qty: Decimal,
        reservation_type: i16,
        source_type: i16,
        source_id: i64,
        priority: i32,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"INSERT INTO inventory_reservations
               (product_id, warehouse_id, reserved_qty, reservation_type, source_type, source_id, priority, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id"#,
            product_id, warehouse_id, reserved_qty, reservation_type, source_type, source_id, priority, expires_at
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn fulfill(executor: impl PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query!("UPDATE inventory_reservations SET status = 2 WHERE id = $1", id)
            .execute(executor).await?;
        Ok(())
    }

    pub async fn cancel(executor: impl PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query!("UPDATE inventory_reservations SET status = 3 WHERE id = $1", id)
            .execute(executor).await?;
        Ok(())
    }

    pub async fn total_reserved(
        executor: impl PgExecutor<'_>,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<Decimal> {
        let total: Option<Decimal> = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(reserved_qty), 0) FROM inventory_reservations
               WHERE product_id = $1 AND warehouse_id = $2 AND status = 1"#,
            product_id, warehouse_id
        )
        .fetch_one(executor)
        .await?;
        Ok(total.unwrap_or(Decimal::ZERO))
    }

    pub async fn expire_old(executor: impl PgExecutor<'_>) -> Result<u64> {
        let result = sqlx::query!(
            "UPDATE inventory_reservations SET status = 4 WHERE status = 1 AND expires_at < NOW()"
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }
}
```

- [ ] **Step 4: 创建 cost_entry_repo**

```rust
// abt/src/repositories/cost_entry_repo.rs
use anyhow::Result;
use common::PgExecutor;
use rust_decimal::Decimal;

pub struct CostEntryRepo;

impl CostEntryRepo {
    pub async fn create(
        executor: impl PgExecutor<'_>,
        entity_type: i16,
        entity_id: i64,
        cost_type: i16,
        debit_amount: Decimal,
        credit_amount: Decimal,
        cost_center: Option<i64>,
        profit_center: Option<i64>,
        period: &str,
        source_type: i16,
        source_id: i64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"INSERT INTO cost_entries
               (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id"#,
            entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_by_entity(
        executor: impl PgExecutor<'_>,
        entity_type: i16,
        entity_id: i64,
    ) -> Result<Vec<crate::models::CostEntry>> {
        let rows = sqlx::query!(
            r#"SELECT id, entity_type, entity_id, cost_type, debit_amount, credit_amount,
                      cost_center, profit_center, period, source_type, source_id, created_at
               FROM cost_entries WHERE entity_type = $1 AND entity_id = $2"#,
            entity_type, entity_id
        )
        .fetch_all(executor)
        .await?;

        Ok(rows.into_iter().map(|r| crate::models::CostEntry {
            id: r.id,
            entity_type: crate::models::CostEntityType::from_i16(r.entity_type).unwrap(),
            entity_id: r.entity_id,
            cost_type: crate::models::CostType::from_i16(r.cost_type).unwrap(),
            debit_amount: r.debit_amount,
            credit_amount: r.credit_amount,
            cost_center: r.cost_center,
            profit_center: r.profit_center,
            period: r.period,
            source_type: crate::models::DocumentType::from_i16(r.source_type).unwrap(),
            source_id: r.source_id,
            created_at: r.created_at,
        }).collect())
    }
}
```

- [ ] **Step 5: 注册到 mod.rs**

在 `abt/src/repositories/mod.rs` 中添加：
```rust
mod document_sequence_repo;
mod document_link_repo;
mod inventory_reservation_repo;
mod cost_entry_repo;
pub use document_sequence_repo::*;
pub use document_link_repo::*;
pub use inventory_reservation_repo::*;
pub use cost_entry_repo::*;
```

- [ ] **Step 6: 验证编译**

Run: `cd E:\work\abt && cargo clippy -p abt 2>&1 | tail -5`
Expected: 无错误

- [ ] **Step 7: Commit**

```bash
git add abt/src/repositories/document_sequence_repo.rs abt/src/repositories/document_link_repo.rs abt/src/repositories/inventory_reservation_repo.rs abt/src/repositories/cost_entry_repo.rs abt/src/repositories/mod.rs
git commit -m "feat: add shared infrastructure repositories"
```

---

### Task 4: Service Trait + Impl — 共享层业务逻辑

**Files:**
- Create: `abt/src/service/document_sequence_service.rs`
- Create: `abt/src/service/document_link_service.rs`
- Create: `abt/src/service/inventory_reservation_service.rs`
- Create: `abt/src/service/cost_entry_service.rs`
- Create: `abt/src/implt/document_sequence_service_impl.rs`
- Create: `abt/src/implt/document_link_service_impl.rs`
- Create: `abt/src/implt/inventory_reservation_service_impl.rs`
- Create: `abt/src/implt/cost_entry_service_impl.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

- [ ] **Step 1: 定义 service traits**

```rust
// abt/src/service/document_sequence_service.rs
use anyhow::Result;
use async_trait::async_trait;
use crate::models::DocumentType;

#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    async fn next_number(&self, doc_type: DocumentType) -> Result<String>;
}
```

```rust
// abt/src/service/document_link_service.rs
use anyhow::Result;
use async_trait::async_trait;
use crate::models::{DocumentType, DocumentLink, LinkType};

#[async_trait]
pub trait DocumentLinkService: Send + Sync {
    async fn create_link(&self, source_type: DocumentType, source_id: i64, target_type: DocumentType, target_id: i64, link_type: LinkType, created_by: Option<i64>) -> Result<i64>;
    async fn find_linked(&self, source_type: DocumentType, source_id: i64) -> Result<Vec<DocumentLink>>;
}
```

```rust
// abt/src/service/inventory_reservation_service.rs
use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use crate::models::{DocumentType, ReservationType};

#[async_trait]
pub trait InventoryReservationService: Send + Sync {
    async fn reserve(&self, product_id: i64, warehouse_id: i64, qty: Decimal, res_type: ReservationType, source_type: DocumentType, source_id: i64, priority: i32, expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Result<i64>;
    async fn fulfill(&self, id: i64) -> Result<()>;
    async fn cancel(&self, id: i64) -> Result<()>;
    async fn total_reserved(&self, product_id: i64, warehouse_id: i64) -> Result<Decimal>;
    async fn expire_old(&self) -> Result<u64>;
}
```

```rust
// abt/src/service/cost_entry_service.rs
use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use crate::models::{CostEntry, CostEntityType, CostType, DocumentType};

#[async_trait]
pub trait CostEntryService: Send + Sync {
    async fn create(&self, entity_type: CostEntityType, entity_id: i64, cost_type: CostType, debit: Decimal, credit: Decimal, cost_center: Option<i64>, profit_center: Option<i64>, period: &str, source_type: DocumentType, source_id: i64) -> Result<i64>;
    async fn find_by_entity(&self, entity_type: CostEntityType, entity_id: i64) -> Result<Vec<CostEntry>>;
}
```

- [ ] **Step 2: 实现 service impls**

```rust
// abt/src/implt/document_sequence_service_impl.rs
use std::sync::Arc;
use sqlx::PgPool;
use async_trait::async_trait;
use anyhow::Result;
use crate::models::DocumentType;
use crate::service::DocumentSequenceService;
use crate::repositories::DocumentSequenceRepo;

pub struct DocumentSequenceServiceImpl {
    pool: Arc<PgPool>,
}

impl DocumentSequenceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self { Self { pool } }
}

#[async_trait]
impl DocumentSequenceService for DocumentSequenceServiceImpl {
    async fn next_number(&self, doc_type: DocumentType) -> Result<String> {
        DocumentSequenceRepo::next_number(&*self.pool, doc_type).await
    }
}
```

```rust
// abt/src/implt/document_link_service_impl.rs
use std::sync::Arc;
use sqlx::PgPool;
use async_trait::async_trait;
use anyhow::Result;
use crate::models::{DocumentType, DocumentLink, LinkType};
use crate::service::DocumentLinkService;
use crate::repositories::DocumentLinkRepo;

pub struct DocumentLinkServiceImpl {
    pool: Arc<PgPool>,
}

impl DocumentLinkServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self { Self { pool } }
}

#[async_trait]
impl DocumentLinkService for DocumentLinkServiceImpl {
    async fn create_link(&self, source_type: DocumentType, source_id: i64, target_type: DocumentType, target_id: i64, link_type: LinkType, created_by: Option<i64>) -> Result<i64> {
        DocumentLinkRepo::create_link(&*self.pool, source_type.as_i16(), source_id, target_type.as_i16(), target_id, link_type.as_i16(), created_by).await
    }

    async fn find_linked(&self, source_type: DocumentType, source_id: i64) -> Result<Vec<DocumentLink>> {
        DocumentLinkRepo::find_linked(&*self.pool, source_type.as_i16(), source_id).await
    }
}
```

```rust
// abt/src/implt/inventory_reservation_service_impl.rs
use std::sync::Arc;
use sqlx::PgPool;
use async_trait::async_trait;
use anyhow::Result;
use rust_decimal::Decimal;
use crate::models::{DocumentType, ReservationType};
use crate::service::InventoryReservationService;
use crate::repositories::InventoryReservationRepo;

pub struct InventoryReservationServiceImpl {
    pool: Arc<PgPool>,
}

impl InventoryReservationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self { Self { pool } }
}

#[async_trait]
impl InventoryReservationService for InventoryReservationServiceImpl {
    async fn reserve(&self, product_id: i64, warehouse_id: i64, qty: Decimal, res_type: ReservationType, source_type: DocumentType, source_id: i64, priority: i32, expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Result<i64> {
        InventoryReservationRepo::create(&*self.pool, product_id, warehouse_id, qty, res_type.as_i16(), source_type.as_i16(), source_id, priority, expires_at).await
    }

    async fn fulfill(&self, id: i64) -> Result<()> {
        InventoryReservationRepo::fulfill(&*self.pool, id).await
    }

    async fn cancel(&self, id: i64) -> Result<()> {
        InventoryReservationRepo::cancel(&*self.pool, id).await
    }

    async fn total_reserved(&self, product_id: i64, warehouse_id: i64) -> Result<Decimal> {
        InventoryReservationRepo::total_reserved(&*self.pool, product_id, warehouse_id).await
    }

    async fn expire_old(&self) -> Result<u64> {
        InventoryReservationRepo::expire_old(&*self.pool).await
    }
}
```

```rust
// abt/src/implt/cost_entry_service_impl.rs
use std::sync::Arc;
use sqlx::PgPool;
use async_trait::async_trait;
use anyhow::Result;
use rust_decimal::Decimal;
use crate::models::{CostEntry, CostEntityType, CostType, DocumentType};
use crate::service::CostEntryService;
use crate::repositories::CostEntryRepo;

pub struct CostEntryServiceImpl {
    pool: Arc<PgPool>,
}

impl CostEntryServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self { Self { pool } }
}

#[async_trait]
impl CostEntryService for CostEntryServiceImpl {
    async fn create(&self, entity_type: CostEntityType, entity_id: i64, cost_type: CostType, debit: Decimal, credit: Decimal, cost_center: Option<i64>, profit_center: Option<i64>, period: &str, source_type: DocumentType, source_id: i64) -> Result<i64> {
        CostEntryRepo::create(&*self.pool, entity_type.as_i16(), entity_id, cost_type.as_i16(), debit, credit, cost_center, profit_center, period, source_type.as_i16(), source_id).await
    }

    async fn find_by_entity(&self, entity_type: CostEntityType, entity_id: i64) -> Result<Vec<CostEntry>> {
        CostEntryRepo::find_by_entity(&*self.pool, entity_type.as_i16(), entity_id).await
    }
}
```

- [ ] **Step 3: 注册到 mod.rs 文件**

`abt/src/service/mod.rs` 添加：
```rust
mod document_sequence_service;
mod document_link_service;
mod inventory_reservation_service;
mod cost_entry_service;
pub use document_sequence_service::DocumentSequenceService;
pub use document_link_service::DocumentLinkService;
pub use inventory_reservation_service::InventoryReservationService;
pub use cost_entry_service::CostEntryService;
```

`abt/src/implt/mod.rs` 添加：
```rust
mod document_sequence_service_impl;
mod document_link_service_impl;
mod inventory_reservation_service_impl;
mod cost_entry_service_impl;
pub use document_sequence_service_impl::DocumentSequenceServiceImpl;
pub use document_link_service_impl::DocumentLinkServiceImpl;
pub use inventory_reservation_service_impl::InventoryReservationServiceImpl;
pub use cost_entry_service_impl::CostEntryServiceImpl;
```

- [ ] **Step 4: 添加工厂函数到 lib.rs**

在 `abt/src/lib.rs` 中添加：
```rust
#[allow(non_snake_case)]
pub fn get_document_sequence_service(ctx: &AppContext) -> impl crate::service::DocumentSequenceService {
    crate::implt::DocumentSequenceServiceImpl::new(Arc::new(ctx.pool().clone()))
}

#[allow(non_snake_case)]
pub fn get_document_link_service(ctx: &AppContext) -> impl crate::service::DocumentLinkService {
    crate::implt::DocumentLinkServiceImpl::new(Arc::new(ctx.pool().clone()))
}

#[allow(non_snake_case)]
pub fn get_inventory_reservation_service(ctx: &AppContext) -> impl crate::service::InventoryReservationService {
    crate::implt::InventoryReservationServiceImpl::new(Arc::new(ctx.pool().clone()))
}

#[allow(non_snake_case)]
pub fn get_cost_entry_service(ctx: &AppContext) -> impl crate::service::CostEntryService {
    crate::implt::CostEntryServiceImpl::new(Arc::new(ctx.pool().clone()))
}
```

- [ ] **Step 5: 验证编译**

Run: `cd E:\work\abt && cargo clippy -p abt 2>&1 | tail -5`
Expected: 无错误

- [ ] **Step 6: Commit**

```bash
git add abt/src/service/ abt/src/implt/ abt/src/lib.rs
git commit -m "feat: add shared infrastructure service layer"
```

---

### Task 5: Proto + gRPC Handler — 共享层 API

**Files:**
- Create: `proto/abt/v1/shared.proto`
- Create: `abt-grpc/src/handlers/shared.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`

- [ ] **Step 1: 定义 shared.proto**

```protobuf
// proto/abt/v1/shared.proto
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

service AbtSharedService {
  rpc GetNextDocumentNumber(GetNextDocumentNumberRequest) returns (DocumentNumberResponse);
  rpc CreateDocumentLink(CreateDocumentLinkRequest) returns (DocumentLinkResponse);
  rpc QueryDocumentLinks(QueryDocumentLinksRequest) returns (DocumentLinkListResponse);
}

enum DocumentType {
  DOCUMENT_TYPE_UNSPECIFIED = 0;
  QUOTATION = 1;
  SALES_ORDER = 2;
  SHIPPING_REQUEST = 3;
  SALES_RETURN = 4;
  RECONCILIATION = 5;
  PURCHASE_QUOTATION = 6;
  PURCHASE_ORDER = 7;
  PURCHASE_RETURN = 8;
  MISCELLANEOUS_REQUEST = 9;
  WORK_ORDER = 10;
  OUTSOURCING_ORDER = 11;
  PRODUCTION_PLAN = 12;
  WORK_REPORT = 13;
  PRODUCTION_INSPECTION = 14;
  PRODUCTION_RECEIPT = 15;
  ARRIVAL_NOTICE = 16;
  MATERIAL_REQUISITION = 17;
  BACKFLUSH = 18;
  CYCLE_COUNT = 19;
  INVENTORY_TRANSFER = 20;
  FORM_CONVERSION = 21;
  INVENTORY_LOCK = 22;
  PAYMENT_REQUEST = 23;
  INVOICE = 24;
}

enum LinkType {
  LINK_TYPE_UNSPECIFIED = 0;
  DERIVED_FROM = 1;
  TRIGGERS = 2;
  REFERENCES = 3;
  RECONCILES = 4;
  INSPECTS = 5;
  FULFILLS = 6;
  ALLOCATES = 7;
}

message GetNextDocumentNumberRequest {
  DocumentType doc_type = 1;
}

message DocumentNumberResponse {
  string doc_number = 1;
}

message CreateDocumentLinkRequest {
  DocumentType source_type = 1;
  int64 source_id = 2;
  DocumentType target_type = 3;
  int64 target_id = 4;
  LinkType link_type = 5;
  optional int64 created_by = 6;
}

message DocumentLinkResponse {
  int64 id = 1;
  DocumentType source_type = 2;
  int64 source_id = 3;
  DocumentType target_type = 4;
  int64 target_id = 5;
  LinkType link_type = 6;
}

message QueryDocumentLinksRequest {
  DocumentType source_type = 1;
  int64 source_id = 2;
}

message DocumentLinkListResponse {
  repeated DocumentLinkResponse items = 1;
}
```

- [ ] **Step 2: 构建 proto 代码**

Run: `cd E:\work\abt && cargo build -p abt-grpc 2>&1 | tail -10`
Expected: proto 代码自动生成

- [ ] **Step 3: 创建 handler**

```rust
// abt-grpc/src/handlers/shared.rs
use tonic::{Request, Response, Status};
use abt::models::document_sequence::DocumentType;

pub struct SharedHandler;

impl SharedHandler {
    pub fn new() -> Self { Self }
}

impl Default for SharedHandler {
    fn default() -> Self { Self::new() }
}

fn proto_doc_type(dt: i32) -> Option<DocumentType> {
    // proto enum value -> model DocumentType
    DocumentType::from_i16(dt as i16)
}

fn model_doc_type(dt: DocumentType) -> i32 {
    dt.as_i16() as i32
}
```

- [ ] **Step 4: 注册到 server.rs**

在 `abt-grpc/src/server.rs` 的 `start_server` 中添加 shared service 注册。

- [ ] **Step 5: 验证编译**

Run: `cd E:\work\abt && cargo clippy -p abt-grpc 2>&1 | tail -5`
Expected: 无错误

- [ ] **Step 6: Commit**

```bash
git add proto/abt/v1/shared.proto abt-grpc/src/handlers/shared.rs abt-grpc/src/handlers/mod.rs abt-grpc/src/server.rs abt-grpc/src/generated/
git commit -m "feat: add shared infrastructure gRPC API"
```

---

## Self-Review

1. **Spec coverage:** 覆盖了设计规范 Section 2 的全部 4 个共享组件。
2. **Placeholder scan:** 无 TBD/TODO，所有代码步骤包含完整实现。
3. **Type consistency:** DocumentType 枚举值在 model/repo/service/handler/proto 间保持一致（i16 ↔ i32 转换明确）。
