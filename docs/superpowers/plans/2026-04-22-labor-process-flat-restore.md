# Labor Process Flat Model Restore — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Revert the three-layer labor process model to a simple flat `bom_labor_process` table with `product_code` association, rewriting all layers while preserving existing code patterns.

**Architecture:** Single flat table `bom_labor_process` per product, with CRUD + Excel import/export. Each layer (migration → proto → model → repo → service → handler) is rewritten independently but deployed atomically due to sqlx compile-time checks.

**Tech Stack:** Rust, sqlx (compile-time checked queries), tonic/prost (gRPC), calamine (Excel read), rust_xlsxwriter (Excel write), PostgreSQL

**Spec:** `docs/superpowers/specs/2026-04-22-labor-process-flat-restore-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `abt/migrations/023_revert_to_flat_labor_process.sql` | Create | Drop three-layer tables, create flat table |
| `proto/abt/v1/labor_process.proto` | Rewrite | Flat model messages and RPCs |
| `abt/src/models/labor_process.rs` | Rewrite | BomLaborProcess and request structs |
| `abt/src/repositories/labor_process_repo.rs` | Rewrite | Flat CRUD + batch operations |
| `abt/src/service/labor_process_service.rs` | Rewrite | Simplified service trait |
| `abt/src/implt/labor_process_service_impl.rs` | Rewrite | Simplified implementation |
| `abt-grpc/src/handlers/labor_process.rs` | Rewrite | Simplified handler |

No changes needed: `lib.rs`, `server.rs`, `mod.rs` files (module names and factory signatures remain compatible).

---

## Task 1: Database Migration

**Files:**
- Create: `abt/migrations/023_revert_to_flat_labor_process.sql`

- [ ] **Step 1: Write the migration SQL**

```sql
-- Migration 023: Revert to flat labor process model
-- Drops three-layer tables (from migration 021) and recreates simple bom_labor_process

BEGIN;

-- ============================================================================
-- 1. Drop three-layer tables (reverse dependency order)
-- ============================================================================
DROP TABLE IF EXISTS bom_labor_cost;
DROP TABLE IF EXISTS labor_process_group_member;
DROP TABLE IF EXISTS labor_process_group;
DROP TABLE IF EXISTS labor_process;

-- Remove process_group_id column from bom table
ALTER TABLE bom DROP COLUMN IF EXISTS process_group_id;

-- Drop archived old table if it exists (from migration 021)
DROP TABLE IF EXISTS bom_labor_process_archived;

-- ============================================================================
-- 2. Create flat bom_labor_process table
-- ============================================================================
CREATE TABLE bom_labor_process (
    id BIGSERIAL PRIMARY KEY,
    product_code VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    unit_price DECIMAL(18,6) NOT NULL,
    quantity DECIMAL(18,6) NOT NULL DEFAULT 1,
    sort_order INT NOT NULL DEFAULT 0,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(product_code, name)
);

CREATE INDEX idx_bom_labor_process_product_code ON bom_labor_process(product_code);

COMMENT ON TABLE bom_labor_process IS 'BOM 人工工序表（按产品管理）';
COMMENT ON COLUMN bom_labor_process.product_code IS '产品编码，关联 BOM 的产品';
COMMENT ON COLUMN bom_labor_process.name IS '工序名称';
COMMENT ON COLUMN bom_labor_process.unit_price IS '工序单价';
COMMENT ON COLUMN bom_labor_process.quantity IS '数量';
COMMENT ON COLUMN bom_labor_process.sort_order IS '排序顺序';
COMMENT ON COLUMN bom_labor_process.remark IS '备注';

COMMIT;
```

- [ ] **Step 2: Verify migration applies**

Run: `cd E:/work/abt && sqlx migrate run -p abt` (or verify SQL syntax manually)
Expected: Migration applies without errors

- [ ] **Step 3: Commit**

```bash
git add abt/migrations/023_revert_to_flat_labor_process.sql
git commit -m "chore: add migration 023 to revert labor process to flat model"
```

---

## Task 2: Proto Definitions

**Files:**
- Rewrite: `proto/abt/v1/labor_process.proto`

- [ ] **Step 1: Write the new proto file**

Replace the entire content of `proto/abt/v1/labor_process.proto` with:

```protobuf
syntax = "proto3";
package abt.v1;

option go_package = "abt/v1";

import "abt/v1/base.proto";
import "abt/v1/excel.proto";

service AbtLaborProcessService {
  // CRUD
  rpc ListLaborProcesses(ListLaborProcessesRequest) returns (LaborProcessListResponse);
  rpc CreateLaborProcess(CreateLaborProcessRequest) returns (U64Response);
  rpc UpdateLaborProcess(UpdateLaborProcessRequest) returns (BoolResponse);
  rpc DeleteLaborProcess(DeleteLaborProcessRequest) returns (U64Response);

  // Excel 导入导出
  rpc ImportLaborProcesses(ImportLaborProcessesRequest) returns (ImportLaborProcessesResponse);
  rpc ExportLaborProcesses(ExportLaborProcessesRequest) returns (stream DownloadFileResponse);
}

// ============================================================================
// 工序消息
// ============================================================================

message BomLaborProcessProto {
  int64 id = 1;
  string product_code = 2;
  string name = 3;
  string unit_price = 4;
  string quantity = 5;
  int32 sort_order = 6;
  string remark = 7;
}

message ListLaborProcessesRequest {
  string product_code = 1;
  optional string keyword = 2;
  optional uint32 page = 3;
  optional uint32 page_size = 4;
}

message LaborProcessListResponse {
  repeated BomLaborProcessProto items = 1;
  uint64 total = 2;
}

message CreateLaborProcessRequest {
  string product_code = 1;
  string name = 2;
  string unit_price = 3;
  string quantity = 4;
  int32 sort_order = 5;
  string remark = 6;
}

message UpdateLaborProcessRequest {
  int64 id = 1;
  string product_code = 2;
  string name = 3;
  string unit_price = 4;
  string quantity = 5;
  int32 sort_order = 6;
  string remark = 7;
}

message DeleteLaborProcessRequest {
  int64 id = 1;
  string product_code = 2;
}

// ============================================================================
// Excel 导入导出
// ============================================================================

message ImportLaborProcessesRequest {
  string file_path = 1;
  string product_code = 2;
}

message ImportLaborProcessesResponse {
  int32 success_count = 1;
  int32 failure_count = 2;
  repeated ImportLaborProcessResult results = 3;
}

message ImportLaborProcessResult {
  int32 row_number = 1;
  string process_name = 2;
  string operation = 3;
  string error_message = 4;
}

message ExportLaborProcessesRequest {
  string product_code = 1;
}
```

- [ ] **Step 2: Verify proto compiles**

Run: `cargo build -p abt-grpc`
Expected: Build succeeds, new proto types generated in `abt-grpc/src/generated/`

- [ ] **Step 3: Commit**

```bash
git add proto/abt/v1/labor_process.proto
git commit -m "refactor: rewrite labor_process.proto for flat model"
```

---

## Task 3: Models

**Files:**
- Rewrite: `abt/src/models/labor_process.rs`

- [ ] **Step 1: Write the new model file**

Replace the entire content of `abt/src/models/labor_process.rs` with:

```rust
//! 劳务工序数据模型
//!
//! 扁平模型：每个产品独立管理自己的工序列表

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// BOM 工序（按产品管理）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomLaborProcess {
    pub id: i64,
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// 请求结构
// ============================================================================

/// 创建工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessReq {
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 更新工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessReq {
    pub id: i64,
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

// ============================================================================
// 查询结构
// ============================================================================

/// 工序查询参数
#[derive(Debug, Clone, Default)]
pub struct ListLaborProcessQuery {
    pub product_code: String,
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

// ============================================================================
// Excel 导入导出
// ============================================================================

/// Excel 列定义常量（导入和导出共用，保证 round-trip 兼容）
pub const LABOR_PROCESS_EXCEL_COLUMNS: &[&str] = &["工序名称", "单价", "数量", "排序", "备注"];

/// 工序 Excel 导入结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportResult {
    pub success_count: i32,
    pub failure_count: i32,
    pub results: Vec<LaborProcessImportRowResult>,
}

/// 工序 Excel 导入单行结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportRowResult {
    pub row_number: i32,
    pub process_name: String,
    pub operation: String, // "created", "updated", "error"
    pub error_message: String,
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p abt`
Expected: Build succeeds (may have unused import warnings from old code, that's OK)

- [ ] **Step 3: Commit**

```bash
git add abt/src/models/labor_process.rs
git commit -m "refactor: rewrite labor process models for flat bom_labor_process"
```

---

Continued in `docs/superpowers/plans/2026-04-22-labor-process-flat-restore-2.md` (Tasks 4-7: Repository, Service, Implementation, Handler)
