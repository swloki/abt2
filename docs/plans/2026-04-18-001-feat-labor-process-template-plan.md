---
title: "feat: Add labor process template management"
type: feat
status: active
date: 2026-04-18
origin: docs/superpowers/specs/2026-04-18-labor-process-template-design.md
---

# feat: Add labor process template management

## Overview

Add a three-layer labor process template system (group → category → step) that decouples labor cost definitions from individual BOMs. BOMs reference template steps; prices are fetched live from templates, eliminating the need to update every BOM when a process price changes.

## Problem Frame

Currently, labor costs are stored per BOM via `bom_labor_process` (linked by `product_code`). When a process price changes, every affected BOM must be updated manually. The new template system centralizes price management so a single update propagates to all referencing BOMs.

## Requirements Trace

- R1. Three-layer hierarchy: group → category (parent_id IS NULL) → step (parent_id references category)
- R2. Each BOM associates with exactly one process group, at step-level granularity with configurable quantity
- R3. Pure reference — BOM stores only step_id + quantity, prices fetched live from template
- R4. Referenced groups/categories/steps cannot be deleted
- R5. All new tables — no migration of old `bom_labor_process` data
- R6. DECIMAL(18,6) consistent with project-wide convention (migration 011)
- R7. Audit trail via `created_by` fields
- R8. SetBomLaborProcess uses idempotent replace semantics with step_id ownership validation
- R9. BOM list responses include associated process group info
- R10. ListProcessGroups supports search/filter/pagination

## Scope Boundaries

- No migration of existing `bom_labor_process` data
- No price change history logging (deferred)
- No soft delete (hard delete with reference check)
- No batch create / clone / reorder operations (v1)

### Deferred to Separate Tasks

- Price change audit log table (`labor_process_price_log`): future iteration
- Batch create items API: can be added without schema changes
- Clone process group: low-frequency, future iteration

## Context & Research

### Relevant Code and Patterns

- **End-to-end reference**: existing `labor_process` feature traces the full pattern — `abt/src/models/labor_process.rs` → `abt/src/repositories/labor_process_repo.rs` → `abt/src/service/labor_process_service.rs` → `abt/src/implt/labor_process_service_impl.rs` → `abt-grpc/src/handlers/labor_process.rs`
- **Repository pattern**: zero-size struct with static methods; `Executor<'_>` param for mutations, `&PgPool` for reads; `sqlx::query!` / `query_as!` / `query_scalar!` for compile-time checked SQL
- **Service pattern**: `#[async_trait]` trait with `Send + Sync`; impl struct takes `PgPool` directly (not `Arc<PgPool>`); factory function in `abt/src/lib.rs`
- **Handler pattern**: standalone public async functions; `AppState::get().await` for state; `state.begin_transaction()` for mutations; proto↔model conversion in handler; Decimal fields as string in proto, parsed in handler
- **BomHandler delegation**: `AbtBomService` impl in `abt-grpc/src/handlers/bom.rs` delegates to `crate::handlers::labor_process::xxx_internal()` functions
- **Permission macro**: `#[require_permission(Resource::X, Action::Y)]` on handler methods; `Resource::LaborProcess = 7` already defined
- **Proto compilation**: `abt-grpc/build.rs` auto-scans `proto/abt/v1/*.proto` — just add file and `cargo build`
- **mod.rs pattern**: `mod xxx;` + `pub use xxx::*;` (models/services/impls) or `pub use xxx::XxxRepo;` (repos) or `pub mod xxx;` (handlers)

### Institutional Learnings

- **Migration safety** (critical): Never TRUNCATE before INSERT. Use `INSERT ... ON CONFLICT DO NOTHING`. Use `ALTER TABLE ... RENAME TO _archived` instead of `DROP TABLE`.
- **Permission proc-macro**: Method-level attributes expand before impl-level `#[tonic::async_trait]`. The macro detects `Box::pin(async move { ... })` wrapper.
- **Permission enum updates**: When adding a new resource, update: (1) proto enum, (2) `PermissionCode` match, (3) `resources.rs` display mappings, (4) consistency test entry.

## Key Technical Decisions

- **Two independent gRPC services** (LaborProcessGroupService + LaborProcessItemService) in one proto file, following the design spec. Group operations include full-tree fetch; item operations are granular CRUD.
- **BOM labor process ref** remains on `AbtBomService` (SetBomLaborProcess/GetBomLaborProcess), following the existing pattern where labor process RPCs live on the BOM service.
- **No foreign key constraints** — all REFERENCES removed per user preference. Referential integrity enforced at application layer.
- **parent_id IS NULL** for categories (not 0 sentinel) — clearer SQL semantics.
- **New permission resource** `LABOR_PROCESS_TEMPLATE = 13` for template management. BOM ref operations use existing `Resource::Bom` permissions.
- **Two separate handler structs**: `LaborProcessGroupHandler` and `LaborProcessItemHandler`, each implementing one generated gRPC service trait. Both delegate to standalone async functions in `labor_process_template.rs`.

## Open Questions

### Resolved During Planning

- Proto service placement: separate new services (not crammed into AbtBomService) per repo research recommendation
- Service impl struct: use `PgPool` directly (not `Arc<PgPool>`), matching `LaborProcessServiceImpl` pattern
- Migration numbering: starts at 021

### Deferred to Implementation

- Exact SQL query text for `find_full_tree` — depends on seeing real data patterns
- Whether `GetBomLaborProcess` response needs the full template tree or just referenced steps — implementation-time UX decision
- Proto message field naming — follow existing conventions (snake_case)

## Implementation Units

- [ ] **Unit 1: Database migrations**

**Goal:** Create all three tables for the labor process template system.

**Requirements:** R1, R5, R6, R7

**Dependencies:** None

**Files:**
- Create: `abt/migrations/021_labor_process_group.sql`
- Create: `abt/migrations/022_labor_process_item.sql`
- Create: `abt/migrations/023_bom_labor_process_ref.sql`

**Approach:**
- Create tables in dependency order: group → item → ref
- `labor_process_group`: id, name (UNIQUE), remark, created_by, timestamps
- `labor_process_item`: id, group_id, parent_id (NULL=category, non-NULL=step), name, unit_price (DECIMAL(18,6)), sort_order, created_by, timestamps. CHECK constraint: (parent_id IS NULL AND unit_price IS NULL) OR (parent_id IS NOT NULL AND unit_price IS NOT NULL). UNIQUE(group_id, parent_id, name). Index on (group_id, parent_id).
- `bom_labor_process_ref`: id, bom_id, step_id, quantity (DECIMAL(18,6)), timestamps. UNIQUE(bom_id, step_id). Index on bom_id and step_id.
- No REFERENCES constraints; use comments to document logical relationships

**Patterns to follow:**
- `abt/migrations/add_bom_labor_process.sql` for table style
- `abt/migrations/011_alter_decimal_scale_to_6.sql` for DECIMAL(18,6) convention

**Test scenarios:**
- Test expectation: none — SQL migrations validated by running against PostgreSQL in development

**Verification:**
- All three tables created successfully; CHECK constraints enforce category/step rules; UNIQUE constraints prevent duplicates

---

- [ ] **Unit 2: Proto definitions**

**Goal:** Define gRPC messages and services for the template system.

**Requirements:** R1, R8, R9, R10

**Dependencies:** None

**Files:**
- Create: `proto/abt/v1/labor_process_template.proto`
- Modify: `proto/abt/v1/bom.proto` (add SetBomLaborProcess/GetBomLaborProcess RPCs + messages, extend BomResponse with group info)
- Modify: `proto/abt/v1/permission.proto` (add LABOR_PROCESS_TEMPLATE = 13)

**Approach:**
- Two separate gRPC service definitions in one proto file: `AbtLaborProcessGroupService` and `AbtLaborProcessItemService`
- In Rust: one `LaborProcessTemplateService` trait backing both proto services (group + item methods in one trait)
- `ListProcessGroupsRequest` includes keyword (optional string), page, page_size
- `ProcessGroupDetailResponse` returns full tree: group → categories → steps with prices
- `SetBomLaborProcessRequest`: bom_id + group_id + repeated StepQuantity(step_id, quantity)
- `BomLaborProcessDetailResponse`: full tree with quantity and subtotal per step
- `BomResponse` / `BomNodeResponse`: add optional `labor_process_group_id` and `labor_process_group_name`
- Decimal fields as string in proto (project convention)
- Import base.proto for shared response types (U64Response, BoolResponse)

**Patterns to follow:**
- `proto/abt/v1/bom.proto` for message naming and structure
- `proto/abt/v1/base.proto` for shared response types
- `proto/abt/v1/permission.proto` for Resource enum

**Test scenarios:**
- Test expectation: none — proto validated by `cargo build` compilation

**Verification:**
- `cargo build` succeeds and generates service stubs in `abt-grpc/src/generated/`

---

- [ ] **Unit 3: Models**

**Goal:** Create Rust model structs for all three tables.

**Requirements:** R1, R6, R7

**Dependencies:** None (model structs use database types only, no proto dependencies)

**Files:**
- Create: `abt/src/models/labor_process_template.rs` (all three structs in one file: ProcessGroup, ProcessItem, BomLaborProcessRef + request structs)
- Modify: `abt/src/models/mod.rs` (add `mod labor_process_template;` + `pub use labor_process_template::*;`)

**Approach:**
- Main structs derive `Debug, Clone, Serialize, Deserialize, FromRow`
- Request structs derive `Debug, Clone, Deserialize`
- `ProcessItem` has `parent_id: Option<i64>` (NULL = category) and `unit_price: Option<Decimal>` (NULL for categories)
- `BomLaborProcessRef` has `bom_id: i64`, `step_id: i64`, `quantity: Decimal`
- Include `ProcessItemTree` struct for tree assembly (category with children steps)
- Include `BomLaborProcessDetail` struct for response with quantity + subtotal

**Patterns to follow:**
- `abt/src/models/labor_process.rs` for struct layout and derive macros

**Test scenarios:**
- Test expectation: none — model structs validated by compilation and downstream usage

**Verification:**
- `cargo build -p abt` compiles without errors

---

- [ ] **Unit 4: Repositories**

**Goal:** Implement SQL queries for all CRUD operations and reference checks.

**Requirements:** R1, R4, R8

**Dependencies:** Unit 1 (tables must exist for sqlx compile-time checks), Unit 3 (model structs)

**Files:**
- Create: `abt/src/repositories/labor_process_template_repo.rs` (group + item + ref repos in one file, or split if too large)
- Modify: `abt/src/repositories/mod.rs` (add module + pub use)

**Approach:**
Three zero-size structs with static methods:

**ProcessGroupRepo:**
- `find_all(pool, keyword, page, page_size)` — with optional keyword filter and pagination
- `find_by_id(pool, id)` — single group lookup
- `insert(executor, name, remark, created_by)` — create group
- `update(executor, id, name, remark)` — update group
- `delete(executor, id)` — hard delete
- `is_referenced_by_bom(pool, id)` — check if any item in this group is referenced by BOM

**ProcessItemRepo:**
- `find_by_group(pool, group_id)` — all items for a group (for tree assembly)
- `insert(executor, group_id, parent_id, name, unit_price, sort_order, created_by)` — create item (category or step)
- `update(executor, id, name, unit_price, sort_order)` — update item (only steps have price)
- `delete(executor, id)` — hard delete single item
- `delete_by_group(executor, group_id)` — delete all items in a group
- `is_step_referenced(pool, step_id)` — check if step is referenced by BOM
- `is_category_referenced(pool, category_id)` — check if any child step is referenced
- `find_step_ids_by_group(pool, group_id)` — get all step IDs in a group (for validation)
- `swap_sort_order(executor, id_1, id_2)` — swap sort_order between two items

**BomLaborProcessRefRepo:**
- `find_by_bom(pool, bom_id)` — get all refs for a BOM
- `replace_all(executor, bom_id, steps[])` — delete old + batch insert new refs in transaction
- `find_group_by_bom(pool, bom_id)` — get the group_id associated with a BOM (via its refs)

Use `sqlx::query!` / `query_as!` / `query_scalar!` for compile-time checked SQL. Use `sqlx::QueryBuilder` for batch insert.

**Patterns to follow:**
- `abt/src/repositories/labor_process_repo.rs` for static method pattern, Executor usage, sqlx macros
- `abt/src/repositories/bom_repo.rs` for pagination pattern

**Test scenarios:**
- Test expectation: none — repos validated by sqlx compile-time checks and integration via service layer

**Verification:**
- `cargo build -p abt` compiles; sqlx validates all queries against the database

---

- [ ] **Unit 5: Services**

**Goal:** Implement business logic for template management and BOM integration.

**Requirements:** R1, R2, R3, R4, R8, R9

**Dependencies:** Unit 3 (models), Unit 4 (repos)

**Files:**
- Create: `abt/src/service/labor_process_template_service.rs` (combined trait for group + item operations)
- Create: `abt/src/implt/labor_process_template_service_impl.rs`
- Modify: `abt/src/service/mod.rs` (add module + pub use)
- Modify: `abt/src/implt/mod.rs` (add module + pub use)
- Modify: `abt/src/lib.rs` (add factory function)
- Modify: `abt/src/service/bom_service.rs` (add set_bom_labor_process + get_bom_labor_process methods)
- Modify: `abt/src/implt/bom_service_impl.rs` (implement new methods)

**Approach:**

**LaborProcessTemplateService** (single service for both group and item):
- `list_groups(keyword, page, page_size) -> (Vec<ProcessGroup>, i64)`
- `get_group_detail(group_id) -> ProcessGroupDetail` (full tree)
- `create_group(name, remark, created_by, executor) -> i64`
- `update_group(id, name, remark, executor) -> ()`
- `delete_group(id, executor) -> ()` — checks reference before deleting
- `create_item(group_id, parent_id, name, unit_price, sort_order, created_by, executor) -> i64`
- `update_item(id, name, unit_price, sort_order, executor) -> ()`
- `delete_item(id, executor) -> ()` — for categories: checks all child steps; for steps: checks direct reference
- `swap_items(id_1, id_2, executor) -> ()`

**BomService extensions:**
- `set_bom_labor_process(bom_id, group_id, steps[], executor) -> ()` — validates group_id exists, validates step_ids belong to group_id, then replaces all refs
- `get_bom_labor_process(bom_id) -> BomLaborProcessDetail` — fetches refs + step prices, computes subtotals, assembles tree

**Impl struct:** `LaborProcessTemplateServiceImpl { pool: PgPool }` with `new(pool: PgPool)`.

**Factory:** `get_labor_process_template_service(ctx: &AppContext) -> impl LaborProcessTemplateService`

**Patterns to follow:**
- `abt/src/service/labor_process_service.rs` for trait pattern
- `abt/src/implt/labor_process_service_impl.rs` for impl pattern (PgPool directly)
- `abt/src/lib.rs` existing factory functions

**Test scenarios:**
- Happy path: create group, create categories and steps, verify tree structure
- Happy path: set BOM labor process, verify refs created
- Happy path: get BOM labor process with computed subtotals
- Edge case: delete category with referenced steps → error
- Edge case: delete group with referenced items → error
- Edge case: set_bom_labor_process with step_id from wrong group → INVALID_ARGUMENT
- Edge case: set_bom_labor_process with non-existent group_id → NOT_FOUND
- Edge case: set_bom_labor_process switches from group A to group B → old refs cleared

**Verification:**
- `cargo build -p abt` succeeds; service methods callable from handlers

---

- [ ] **Unit 6: Handlers and registration**

**Goal:** Wire up gRPC handlers, permissions, and server registration.

**Requirements:** All

**Dependencies:** Unit 5 (services)

**Files:**
- Create: `abt-grpc/src/handlers/labor_process_template.rs` (standalone async functions for group + item RPCs + BOM ref RPCs)
- Create: `abt-grpc/src/handlers/labor_process_group_handler.rs` (handler struct implementing AbtLaborProcessGroupService trait)
- Create: `abt-grpc/src/handlers/labor_process_item_handler.rs` (handler struct implementing AbtLaborProcessItemService trait)
- Modify: `abt-grpc/src/handlers/mod.rs` (add pub mod declarations)
- Modify: `abt-grpc/src/handlers/bom.rs` (add set_bom_labor_process and get_bom_labor_process methods)
- Modify: `abt-grpc/src/server.rs` (add service registration + AppState method)
- Modify: `abt-grpc/src/permissions/mod.rs` (add PermissionCode match for LaborProcessTemplate)
- Modify: `abt/src/models/resources.rs` (add ResourceActionDef for labor_process_template)

**Approach:**

**Handler files:**
- `labor_process_template.rs`: standalone public async functions following existing pattern — AppState::get(), create service, begin_transaction for mutations, proto↔model conversion, Decimal string parsing
- `labor_process_group_handler.rs`: `LaborProcessGroupHandler` struct implementing `AbtLaborProcessGroupService` trait with `#[require_permission]` annotations, delegating to standalone functions
- `labor_process_item_handler.rs`: `LaborProcessItemHandler` struct implementing `AbtLaborProcessItemService` trait with `#[require_permission]` annotations, delegating to standalone functions

**Permission setup:**
- Template CRUD: `#[require_permission(Resource::LaborProcessTemplate, Action::Read/Write/Delete)]`
- BOM labor process ops: `#[require_permission(Resource::Bom, Action::Read/Write)]`
- Add `Resource::LaborProcessTemplate => "labor_process_template"` to PermissionCode impl
- Add ResourceActionDef to RESOURCES slice

**Server registration:**
- Add `AbtLaborProcessGroupServiceServer::with_interceptor(LaborProcessTemplateGroupHandler::new(), auth_interceptor)`
- Add `AbtLaborProcessItemServiceServer::with_interceptor(LaborProcessTemplateItemHandler::new(), auth_interceptor)`
- Add `labor_process_template_service()` method to AppState

**BomResponse extension:**
- In `list_boms` and `get_bom` handlers, LEFT JOIN to get associated process group info and populate new fields

**Patterns to follow:**
- `abt-grpc/src/handlers/labor_process.rs` for standalone function pattern
- `abt-grpc/src/handlers/bom.rs` for BomHandler trait impl and delegation pattern
- `abt-grpc/src/server.rs` for service registration
- `abt-grpc/src/permissions/mod.rs` for PermissionCode impl
- `abt/src/models/resources.rs` for ResourceActionDef entries

**Test scenarios:**
- Happy path: full gRPC round-trip — create group, create items, set BOM process, get BOM process
- Error path: delete referenced item returns permission/constraint error
- Error path: set_bom_labor_process with invalid step_id returns INVALID_ARGUMENT
- Permission: unauthenticated request rejected
- Permission: unauthorized role rejected

**Verification:**
- `cargo build` succeeds (all crates)
- Server starts without errors
- gRPC reflection shows new services

---

## System-Wide Impact

- **Interaction graph:** New services are self-contained. BomService gains 2 new methods. BomHandler gains 2 new RPC implementations. BOM list query changes to include LEFT JOIN for group info.
- **Error propagation:** Service layer returns `anyhow::Result`; handlers convert via `error::err_to_status`. Reference check failures return `FAILED_PRECONDITION`. Step validation failures return `INVALID_ARGUMENT`.
- **State lifecycle risks:** `replace_bom_processes` is transactional (delete + batch insert). If transaction fails midway, no partial state is committed.
- **API surface parity:** Old `bom_labor_process` RPCs remain unchanged. New template RPCs are additive.
- **Integration coverage:** End-to-end flow (create template → set BOM → get BOM cost) should be tested via gRPC client or integration test.
- **Unchanged invariants:** Existing BOM CRUD, node management, Excel import/export, and old labor process APIs are unaffected.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| sqlx compile-time checks require running PostgreSQL | Ensure DATABASE_URL is set during development; migrations must be applied before building |
| BOM list query performance with LEFT JOIN | Monitor query plan; add index on bom_labor_process_ref(bom_id) |
| Template deletion race condition (check-then-delete) | Application-layer check is sufficient for single-server deployment; transaction provides atomicity |
| Proto regeneration conflicts | build.rs auto-generates; never manually edit generated files |

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-04-18-labor-process-template-design.md](docs/superpowers/specs/2026-04-18-labor-process-template-design.md)
- **Ideation review:** [docs/ideation/2026-04-18-labor-process-template-review-ideation.md](docs/ideation/2026-04-18-labor-process-template-review-ideation.md)
- Reference implementation: `abt/src/models/labor_process.rs` → `abt-grpc/src/handlers/labor_process.rs`
- BOM service: `abt/src/service/bom_service.rs` → `abt-grpc/src/handlers/bom.rs`
- Permission macro: `abt-macros/src/lib.rs`
- Migration safety: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`
