---
title: "feat: Labor Process Redesign"
type: feat
status: active
date: 2026-04-18
origin: docs/superpowers/specs/2026-04-18-labor-process-redesign-design.md
ideation: docs/ideation/2026-04-18-labor-process-redesign-ideation.md
---

# feat: Labor Process Redesign

## Overview

Replace the existing flat `bom_labor_process` table (per-BOM labor configuration via `product_code`) with a three-layer model: global process master list, process groups with join-table membership, and per-BOM cost items with price snapshots. This enables centralized pricing, reusable process groups, and historical cost audit.

## Problem Frame

Current labor cost management requires each BOM to configure labor processes individually. When a labor cost changes, every BOM using that process must be updated manually. The new design introduces a global process list with centralized pricing and process groups for reuse across BOMs, while preserving price history via snapshots.

## Requirements Trace

- R1. Global process master list with unique name and centralized unit price
- R2. Process groups that bundle multiple processes in a defined order (join table with sort_order)
- R3. Per-BOM labor cost items referencing processes with quantities and price snapshots
- R4. BOM table links to a process group via `process_group_id`
- R5. Deleting a process is rejected if any group references it (FK RESTRICT)
- R6. Deleting a process group is rejected if any BOM references it (application-level check)
- R7. `SetBomLaborCost` freezes current master price into `unit_price_snapshot` for audit trail
- R8. `GetBomLaborCost` returns both current and snapshot prices with subtotals
- R9. `UpdateLaborProcess` returns affected BOM count when price changes
- R10. `SetBomLaborCost` requires remark when quantity is 0

## Scope Boundaries

- Excel import functionality from current impl is **temporarily deferred** — to be re-implemented in a future iteration after core CRUD is stable
- Recursive BOM cost rollup API is deferred to future work
- Resource type generalization (machine/overhead) is deferred
- Per-BOM price override is deferred
- cost_source enum (standard/actual/estimated) is deferred

### Deferred to Separate Tasks

- Excel import for labor processes: future iteration after core CRUD is stable
- Recursive BOM cost rollup API: requires the three-layer model to be in place first
- Old `bom_labor_process` data migration: archived table preserved for manual migration if needed

## Context & Research

### Relevant Code and Patterns

- **Existing labor process code (to replace):** `abt/src/models/labor_process.rs`, `abt/src/repositories/labor_process_repo.rs`, `abt/src/service/labor_process_service.rs`, `abt/src/implt/labor_process_service_impl.rs`, `abt-grpc/src/handlers/labor_process.rs`
- **Proto:** Current labor process RPCs defined in `proto/abt/v1/bom.proto` lines 174-228, under `AbtBomService`
- **Join table pattern:** `user_roles` table — composite PK `(user_id, role_id)`, FK CASCADE, indexed FK columns
- **Service factory:** `abt/src/lib.rs` — `get_labor_process_service(&AppContext)` pattern
- **Handler registration:** `abt-grpc/src/server.rs` — tonic service server registration
- **Permission macro:** `#[require_permission(Resource::LaborProcess, Action::Read)]` already exists
- **BOM model:** `abt/src/models/bom.rs` — JSONB `bom_detail`, custom `FromRow`

### Institutional Learnings

- **Migration safety (CRITICAL):** Archive old tables via `ALTER TABLE RENAME TO _archived` instead of DROP; use `INSERT ON CONFLICT DO NOTHING` for data migration (see `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`)
- **Permission macro:** `#[require_permission]` uses enum paths (e.g., `Resource::LaborProcess`), must work with `#[tonic::async_trait]` via `Box::pin` penetration (see `docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md`)
- **Proto enums:** Proto is the single source of truth for `Resource` and `Action` enums (see `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`)

### External References

- None needed — codebase has strong local patterns for all layers involved

## Key Technical Decisions

- **Single proto service** for process, group, and BOM cost: reduces file scatter, keeps tightly coupled operations together (see origin: design spec)
- **Join table over JSONB** for process group membership: provides FK referential integrity, explicit sort_order, and standard SQL queryability (see ideation: idea #1)
- **Price snapshot in bom_labor_cost** instead of live-only: preserves historical cost for audit trail while still computing current cost live (see ideation: idea #2)
- **DECIMAL(18,6)** for all numeric fields: matches codebase convention established by migration 011 (spec originally said 12,6, corrected to match convention)
- **Archive old table** instead of DROP: `bom_labor_process` → `bom_labor_process_archived` per migration safety learning
- **Independent proto service** `AbtLaborProcessService` replacing RPCs currently nested in `AbtBomService`

## Open Questions

### Resolved During Planning

- Proto service structure: single `AbtLaborProcessService` covering all three areas (process CRUD, group CRUD, BOM cost)
- DECIMAL precision: `DECIMAL(18,6)` to match codebase convention
- Migration numbering: `021_labor_process_redesign.sql`
- Old data migration: archive table, no automatic data migration (data model is fundamentally different: `product_code` → `bom_id`)

### Deferred to Implementation

- Exact proto message field names and numbers: depends on reading current `bom.proto` labor process messages for compatibility considerations
- Whether `bom_labor_cost.bom_id` should have a FK constraint to `bom` table: depends on how BOM deletion works in current code
- Index strategy for `bom_labor_cost` beyond the primary key: depends on query patterns observed during implementation

## Implementation Units

- [ ] **Unit 1: Database Migration**

**Goal:** Create the four new tables (`labor_process`, `labor_process_group`, `labor_process_group_member`, `bom_labor_cost`), alter BOM table, and archive old `bom_labor_process`.

**Requirements:** R1, R2, R3, R4, R5

**Dependencies:** None

**Files:**
- Create: `abt/migrations/021_labor_process_redesign.sql`

**Approach:**
- Wrap entire migration in `BEGIN ... COMMIT` transaction
- Create `labor_process` table with UNIQUE constraint on `name`
- Create `labor_process_group` table with UNIQUE constraint on `name`
- Create `labor_process_group_member` join table with composite PK `(group_id, process_id)`, FK to `labor_process_group(id) ON DELETE CASCADE`, FK to `labor_process(id) ON DELETE RESTRICT`, and `sort_order INT NOT NULL`
- Create indexes on `labor_process_group_member.process_id` (for reverse lookup) and `labor_process_group_member.group_id`
- Create `bom_labor_cost` table with FK considerations for `bom_id` and `process_id`
- Create indexes on `bom_labor_cost.bom_id` and `bom_labor_cost.process_id`
- Add `process_group_id BIGINT` column to `bom` table (nullable, no FK constraint yet — BOM table is managed differently)
- Archive old table: `ALTER TABLE bom_labor_process RENAME TO bom_labor_process_archived`
- Add Chinese comments via `COMMENT ON TABLE/COLUMN`
- **Verify** migrations 019 and 020 are applied before running 021

**Patterns to follow:**
- Migration 019/020 style: `BEGIN; ... COMMIT;` wrapping, `IF EXISTS`/`IF NOT EXISTS` protection
- Join table pattern from `user_roles`: composite PK, FK CASCADE, indexed FK columns
- `DECIMAL(18,6)` for all numeric fields per migration 011 convention

**Test scenarios:**
- Happy path: migration applies cleanly via `sqlx migrate run`
- Edge case: running migration twice is idempotent (IF NOT EXISTS guards)
- Edge case: `labor_process_group_member` FK RESTRICT prevents deleting a process that is in a group
- Edge case: `labor_process_group_member` FK CASCADE removes members when group is deleted

**Verification:**
- `cargo build` compiles without errors (sqlx compile-time checks pass against migrated database)
- New tables exist with correct schema
- Old `bom_labor_process` is archived, not dropped
- FK constraints behave as specified (RESTRICT on process delete, CASCADE on group delete)

---

- [ ] **Unit 2: Proto Definitions**

**Goal:** Define `AbtLaborProcessService` proto with all CRUD messages and RPC methods for process, group, and BOM cost. Remove old labor process RPCs from `AbtBomService`.

**Requirements:** R1-R10

**Dependencies:** None (proto and migration are independent foundation work)

**Files:**
- Create: `proto/abt/v1/labor_process.proto`
- Modify: `proto/abt/v1/bom.proto` (remove old labor process messages and RPCs)

**Approach:**
- Create `labor_process.proto` importing `base.proto`
- Define messages: `LaborProcessProto`, `LaborProcessGroupProto` (with `repeated ProcessGroupMemberProto members`), `ProcessGroupMemberProto` (process_id + sort_order), `BomLaborCostProto` (with current_price, snapshot_price, quantity, subtotal, snapshot_subtotal)
- Define standard CRUD request/response messages following `{Entity}{Action}Request/Response` convention
- Define `AbtLaborProcessService` with RPCs: `ListLaborProcesses`, `CreateLaborProcess`, `UpdateLaborProcess` (with `affected_bom_count` and `affected_item_count` in response), `DeleteLaborProcess`, `ListLaborProcessGroups`, `CreateLaborProcessGroup`, `UpdateLaborProcessGroup`, `DeleteLaborProcessGroup`, `SetBomLaborCost`, `GetBomLaborCost`
- Decimal fields use `string` type in proto (existing convention)
- Remove old `BomLaborProcessProto`, `ListLaborProcessesRequest/Response`, `CreateLaborProcessRequest/Response`, `UpdateLaborProcessRequest/Response`, `DeleteLaborProcessRequest/Response` from `bom.proto`
- Remove corresponding RPC methods from `AbtBomService` in `bom.proto`

**Patterns to follow:**
- `proto/abt/v1/bom.proto` for message naming convention
- `proto/abt/v1/product.proto` for service structure convention
- Decimal as `string` type
- `optional uint32 page` and `optional uint32 page_size` for pagination
- `uint64 total` in list responses

**Test scenarios:**
- Test expectation: none — proto definitions are validated at compile time by `tonic`/`prost` build

**Verification:**
- `cargo build -p abt-grpc` compiles successfully with new proto (generates `abt-grpc/src/generated/abt.v1.labor_process.rs`)
- Old labor process RPCs no longer exist in `AbtBomService`

---

- [ ] **Unit 3: Models + Repository**

**Goal:** Define Rust model structs for the four new tables and implement repository with sqlx compile-time checked queries for all CRUD operations.

**Requirements:** R1, R2, R3, R4, R5, R6, R7, R10

**Dependencies:** Unit 1 (migration), Unit 2 (proto for model field mapping)

**Files:**
- Replace: `abt/src/models/labor_process.rs`
- Replace: `abt/src/repositories/labor_process_repo.rs`
- Modify: `abt/src/models/bom.rs` (add `process_group_id` field, update custom `FromRow` impl to include new column)
- Modify: `abt/src/repositories/bom_repo.rs` (update SELECT statements to include `process_group_id`)
- Modify: `abt-grpc/src/handlers/convert.rs` (add `process_group_id` to BOM proto conversion if applicable)
- Modify: `abt/src/models/mod.rs`, `abt/src/repositories/mod.rs` (module registration)

**Approach:**
- Define `LaborProcess` struct with `FromRow` derive (id, name, unit_price, remark, created_at, updated_at)
- Define `LaborProcessGroup` struct (id, name, remark, created_at, updated_at)
- Define `LaborProcessGroupMember` struct (group_id, process_id, sort_order)
- Define `BomLaborCost` struct (id, bom_id, process_id, quantity, unit_price_snapshot, remark, created_at, updated_at)
- Add `process_group_id: Option<i64>` to BOM model
- Repository functions:
  - Process: `list_processes(pool, page, page_size)`, `create_process(executor, name, unit_price, remark)`, `update_process(executor, id, name, unit_price, remark)`, `delete_process(executor, id)`, `get_process(pool, id)`
  - Group: `list_groups(pool, page, page_size)` (JOIN members), `create_group(executor, name, remark)`, `update_group(executor, id, name, remark)`, `delete_group(executor, id)`, `get_group_with_members(pool, id)`
  - Group Member: `set_group_members(executor, group_id, members: Vec<(process_id, sort_order)>)` (delete old + bulk insert in transaction)
  - BOM Cost: `set_bom_labor_costs(executor, bom_id, process_group_id, items: Vec<(process_id, quantity, unit_price_snapshot, remark)>)` (clear old + bulk insert), `get_bom_labor_costs(pool, bom_id)` (JOIN with labor_process for current prices)
  - Impact count: `count_affected_bom_items(pool, process_id)` — count bom_labor_cost items referencing a process
- Use `sqlx::query!` / `sqlx::query_as!` / `sqlx::query_scalar!` for compile-time checks
- Use `sqlx::QueryBuilder` for bulk inserts
- Pagination: `page.max(1) - 1) * page_size.clamp(1, 100)` pattern

**Patterns to follow:**
- Existing `abt/src/repositories/labor_process_repo.rs` for query style and pagination
- `abt/src/repositories/` for Executor pattern (pool for reads, Executor for writes)
- `user_roles` table pattern for join table operations (delete + bulk insert in transaction)
- `DECIMAL` handling via `rust_decimal::Decimal`

**Test scenarios:**
- Happy path: create process, create group with members, set BOM cost, read back with correct prices
- Happy path: list processes with pagination returns correct page
- Edge case: create process with duplicate name returns error
- Edge case: delete process referenced by a group fails (FK RESTRICT)
- Edge case: set BOM cost with quantity=0 and empty remark should be validated at service layer
- Edge case: `count_affected_bom_items` returns 0 when no BOM uses the process
- Edge case: `set_group_members` replaces all existing members atomically

**Verification:**
- `cargo build -p abt` compiles with sqlx compile-time checks passing
- `cargo test -p abt` passes

---

- [ ] **Unit 4: Service Trait + Implementation**

**Goal:** Define the `LaborProcessService` async trait covering all business operations and implement it with validation, price snapshot logic, and affected BOM counting.

**Requirements:** R1-R10

**Dependencies:** Unit 3 (models and repository)

**Files:**
- Replace: `abt/src/service/labor_process_service.rs`
- Replace: `abt/src/implt/labor_process_service_impl.rs`
- Modify: `abt/src/service/mod.rs`, `abt/src/implt/mod.rs` (module registration)
- Modify: `abt/src/lib.rs` (factory function update)

**Approach:**
- Define `LaborProcessService` trait with `#[async_trait]` + `Send + Sync`:
  - `list_processes(&self, page, page_size) -> Result<(Vec<LaborProcess>, u64)>`
  - `create_process(&self, executor, name, unit_price, remark) -> Result<LaborProcess>`
  - `update_process(&self, executor, id, name, unit_price, remark) -> Result<(LaborProcess, u64, u64)>` (returns updated process + affected_bom_count + affected_item_count)
  - `delete_process(&self, executor, id) -> Result<()>`
  - `list_groups(&self, page, page_size) -> Result<(Vec<LaborProcessGroup>, u64)>`
  - `create_group(&self, executor, name, member_process_ids: Vec<i64>, remark) -> Result<LaborProcessGroup>`
  - `update_group(&self, executor, id, name, member_process_ids: Vec<i64>, remark) -> Result<LaborProcessGroup>`
  - `delete_group(&self, executor, id) -> Result<()>`
  - `set_bom_labor_cost(&self, executor, bom_id, process_group_id, items) -> Result<()>`
  - `get_bom_labor_cost(&self, bom_id) -> Result<BomLaborCostDetail>` (returns group + items with current/snapshot prices)
- Implementation:
  - `create_process`: validate unique name, insert
  - `update_process`: detect price change, call `count_affected_bom_items` if price changed, update
  - `delete_process`: FK RESTRICT handles rejection; wrap in descriptive error
  - `create_group`: validate all process_ids exist (query labor_process), create group + set members
  - `update_group`: validate all process_ids exist, update group + replace members
  - `delete_group`: check if any BOM references this group (query bom table for process_group_id), reject if found
  - `set_bom_labor_cost`: validate process_group_id exists, for each item validate process exists, if quantity=0 validate remark is not empty, fetch current prices from labor_process, clear old bom_labor_cost records, bulk insert new records with price snapshots, update bom.process_group_id
  - `get_bom_labor_cost`: query bom_labor_cost JOIN labor_process, compute subtotals (current and snapshot), return aggregated result
- Constructor takes `PgPool` (matching existing `LaborProcessServiceImpl::new` pattern)

**Patterns to follow:**
- Existing `abt/src/service/labor_process_service.rs` for trait style
- Existing `abt/src/implt/labor_process_service_impl.rs` for impl structure
- `abt/src/lib.rs` `get_labor_process_service` factory function
- `anyhow::Result<T>` for error handling
- Write methods accept `Executor<'_>` for transaction propagation

**Test scenarios:**
- Happy path: full lifecycle — create process → create group → set BOM cost → get BOM cost → verify prices
- Happy path: update process price → verify affected count returned
- Error path: create process with duplicate name → rejected
- Error path: delete process in a group → FK violation propagates as clear error
- Error path: delete group referenced by BOM → application-level rejection
- Error path: set BOM cost with quantity=0 and empty remark → validation error
- Edge case: set BOM cost freezes correct price snapshot
- Edge case: get BOM cost returns different current vs snapshot prices after master price update

**Verification:**
- `cargo build -p abt` compiles
- `cargo test -p abt` passes
- All validation rules from spec are enforced

---

- [ ] **Unit 5: gRPC Handler + Registration**

**Goal:** Implement the tonic service handler for `AbtLaborProcessService`, wire it into the server, and clean up old handler code.

**Requirements:** R1-R10

**Dependencies:** Unit 2 (proto), Unit 4 (service)

**Files:**
- Replace: `abt-grpc/src/handlers/labor_process.rs`
- Modify: `abt-grpc/src/handlers/mod.rs` (module registration)
- Modify: `abt-grpc/src/handlers/bom.rs` (remove old labor process RPC delegation)
- Modify: `abt-grpc/src/server.rs` (register new service)
- Modify: `abt-grpc/src/handlers/convert.rs` (if adding From<> impls)

**Approach:**
- Create `LaborProcessHandler` struct (new, not modifying existing) implementing the generated tonic trait for `AbtLaborProcessService`. Unlike the current labor process code which uses standalone helper functions called from `BomHandler`, the new handler is a full tonic trait implementation on its own struct. Constructor follows `BomHandler::new()` pattern — stateless, accesses `AppState::get().await` in each method.
- Each RPC method:
  - Read operations: call service method directly (no transaction)
  - Write operations: `state.begin_transaction()` → service call → `tx.commit()`
  - Add `#[require_permission(Resource::LaborProcess, Action::Read/Write)]` to each method
- Model-to-proto conversion: inline mapping in handler (matching current labor_process handler style) or add `From<>` impls in `convert.rs`
- Decimal handling: `string` in proto ↔ `Decimal` in Rust via `parse()` and `.to_string()`
- Register `AbtLaborProcessServiceServer::new(LaborProcessHandler::new(state))` in `server.rs`
- Remove old labor process RPC methods from `BomHandler` in `bom.rs`
- Remove old internal helper functions from previous `labor_process.rs` handler

**Patterns to follow:**
- `abt-grpc/src/handlers/bom.rs` for tonic trait impl style and transaction management
- `abt-grpc/src/handlers/labor_process.rs` for Decimal conversion and error mapping
- `abt-grpc/src/server.rs` for service registration
- `#[require_permission(Resource::LaborProcess, Action::Read)]` macro usage
- `error::err_to_status` and `error::validation` for error conversion

**Test scenarios:**
- Happy path: gRPC round-trip — create process via gRPC → list processes → verify response
- Happy path: create group → set BOM cost → get BOM cost → verify all fields (current price, snapshot, subtotals, total)
- Happy path: update process price → verify response contains affected_bom_count > 0
- Error path: delete process in use → gRPC error with descriptive message
- Error path: set BOM cost with quantity=0 and no remark → validation error
- Integration: handler correctly propagates service errors as gRPC status codes

**Verification:**
- `cargo build -p abt-grpc` compiles and generates correct proto code
- `cargo test -p abt-grpc` passes
- New `AbtLaborProcessService` responds to gRPC reflection queries
- Old labor process RPCs removed from `AbtBomService`

---

- [ ] **Unit 6: Cleanup and Module Registration**

**Goal:** Ensure all module registrations, factory functions, and re-exports are correct. Remove any dead code from the old implementation.

**Requirements:** All (final wiring)

**Dependencies:** Unit 5 (handler)

**Files:**
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`
- Modify: `abt-grpc/src/handlers/mod.rs` (add `AbtLaborProcessServiceServer` re-export from generated code)
- Modify: `abt/src/implt/product_excel_service_impl.rs` (update `export_boms_without_labor_cost_to_bytes` query to use new `bom_labor_cost` table instead of archived `bom_labor_process`)

**Approach:**
- Verify all `mod` declarations and `pub use` re-exports are updated
- Verify `get_labor_process_service` factory returns the new trait
- Remove references to old model types (`BomLaborProcess`) from bom model if any
- Ensure `AppState::labor_process_service()` returns the correct service type
- Full workspace build verification

**Patterns to follow:**
- Existing `mod.rs` patterns in each directory
- Existing factory functions in `lib.rs`

**Test scenarios:**
- Test expectation: none — pure wiring verification

**Verification:**
- `cargo build` (full workspace) compiles without warnings
- `cargo test` passes all tests
- No references to old `BomLaborProcess` model remain in codebase

## System-Wide Impact

- **Interaction graph:** Old `AbtBomService` RPCs for labor process are removed — any client calling those RPCs will get `Unimplemented`. Clients must migrate to new `AbtLaborProcessService` RPCs.
- **Error propagation:** FK violations from PostgreSQL propagate through sqlx as errors → wrapped in `anyhow::Error` → converted to gRPC status by handler. PostgreSQL FK error code `23503` must be intercepted and converted to semantic gRPC errors (e.g., `FailedPrecondition` or `InvalidArgument`) rather than generic `Internal`. Application-level validations (delete group with BOM references, quantity=0 without remark) return specific error messages.
- **State lifecycle risks:** `SetBomLaborCost` uses clear-then-bulk-insert within a transaction — no partial write risk. `SetGroupMembers` similarly atomic.
- **API surface parity:** The old `AbtBomService` labor process RPCs are removed. New `AbtLaborProcessService` provides equivalent functionality with expanded features (groups, price snapshots, affected count).
- **Integration coverage:** Full gRPC round-trip tests should verify proto↔model conversion, especially for Decimal fields and the new snapshot/current price dual fields.
- **Unchanged invariants:** BOM CRUD operations, product management, inventory queries, and permission system are not affected by this change.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Proto breaking change — old clients call removed RPCs | Coordinate client migration; old RPCs clearly removed from proto |
| sqlx compile-time checks require running database | Ensure `DATABASE_URL` is set and migration has been applied before building |
| Old `bom_labor_process` data not migrated to new tables | Archive table preserved; document manual migration path if needed |
| `SetBomLaborCost` clear-then-insert may cause brief empty read | Operations are within a transaction; concurrent reads see consistent state |
| DECIMAL precision mismatch between spec and codebase | Corrected to `DECIMAL(18,6)` to match codebase convention |
| All units must deploy atomically (sqlx compile-time checks) | Units 1-6 must be implemented in a single commit/PR; partial deployment breaks `cargo build` because archiving old table invalidates all `sqlx::query!` references to `bom_labor_process` |

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-04-18-labor-process-redesign-design.md](docs/superpowers/specs/2026-04-18-labor-process-redesign-design.md)
- **Ideation:** [docs/ideation/2026-04-18-labor-process-redesign-ideation.md](docs/ideation/2026-04-18-labor-process-redesign-ideation.md)
- Migration safety learning: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`
- Permission macro: `docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md`
- Join table pattern: `user_roles` table in existing migrations
