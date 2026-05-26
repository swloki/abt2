---
title: feat: Add warehouse/location Excel import-export
type: feat
status: active
date: 2026-05-03
origin: docs/superpowers/specs/2026-05-03-warehouse-location-excel-design.md
---

# feat: Add warehouse/location Excel import-export

## Summary

Add Excel import and export for warehouses and their locations. Import uses a flat Excel format (warehouse_code, warehouse_name, location_code, location_name, capacity) with code-based upsert semantics. Export dumps all non-deleted warehouses and locations in the same format. Both reuse the existing `AbtExcelService` gRPC infrastructure.

---

## Problem Frame

Warehouse and location master data currently has no bulk import/export path. Users must create warehouses and locations one-by-one through CRUD RPCs, which is impractical for initial setup (hundreds of locations per warehouse) or periodic synchronization with external systems. The codebase already has an Excel import/export infrastructure (AbtExcelService, ExcelImportService/ExcelExportService traits) used by product inventory and labor process features — this feature extends that infrastructure to warehouse/location data.

---

## Requirements

- R1. Import warehouses and locations from a flat Excel file with code-based upsert
- R2. Export all non-deleted warehouses and locations to an Excel file matching the import format
- R3. Validate input data before any database writes (warehouse name consistency, code rename detection, soft-delete conflict detection)
- R4. Report per-row structured errors (row index, column name, reason, raw value)
- R5. Support optional sync_mode that soft-deletes locations not present in the import file
- R6. Replace string-based import_type/export_type dispatch with typed enums

---

## Scope Boundaries

- This feature does NOT add new gRPC methods — it reuses existing AbtExcelService
- The sync_mode (R5) is optional and disabled by default
- Warehouse-level sync (deleting warehouses not in file) is NOT included
- Excel template download is NOT included — import format is documented only

---

## Context & Research

### Relevant Code and Patterns

- **Existing import pattern:** `abt/src/implt/excel/product_inventory_import.rs` — two-phase parse-then-upsert, `ExcelImportService` trait, `ProgressTracker` integration
- **Existing export pattern:** `abt/src/implt/excel/product_all_export.rs` — `ExcelExportService<Params = ()>`, `write_headers()`, `Workbook::save_to_buffer()`
- **Shared helpers:** `abt/src/implt/excel/mod.rs` — `import_range_from_source()`, `write_headers()`, `deserialize_optional_decimal()`
- **Progress tracking:** `abt/src/implt/excel/progress.rs` — `ProgressTracker` (AtomicUsize-based, shared via Arc)
- **gRPC handler:** `abt-grpc/src/handlers/excel.rs` — string-based dispatch, `active_imports` HashMap
- **Service traits:** `abt/src/service/excel_service.rs` — `ExcelImportService` + `ExcelExportService`
- **Proto definition:** `proto/abt/v1/excel.proto` — `AbtExcelService` RPCs
- **Proto compilation:** `abt-grpc/build.rs` — auto-scans `proto/abt/v1/`
- **Warehouse repo:** `abt/src/repositories/warehouse_repo.rs` — `find_by_code`, `insert`, `update`
- **Location repo:** `abt/src/repositories/location_repo.rs` — `find_by_code`, `insert`, `update`, `list_all_with_warehouse`
- **Re-export path:** `abt/src/lib.rs` line 30: `pub use implt::excel;`

### Institutional Learnings

- **active_imports concurrency trap:** The handler's `Mutex<HashMap<String, Arc<ProgressTracker>>>` uses string keys — two concurrent imports of the same type collide. The typed enum (R6) mitigates this but does not fully solve it; per-request UUID keys would be needed for full concurrency safety (see `docs/plans/2026-04-29-001-refactor-excel-service-unification-plan.md` review notes).
- **ON CONFLICT + soft-delete gap:** `warehouse` table has `UNIQUE(warehouse_code)` without `WHERE deleted_at IS NULL`. Soft-deleted records block recreation with the same code. Phase 1 must detect this before entering the transaction.
- **`business_error` for validation:** Import validation failures (bad data, naming conflicts) should use `error::business_error()` (zero log output) rather than `error::err_to_status()` (logs backtrace). See `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`.
- **Shared column constants:** Import and export should share a single `pub const WAREHOUSE_LOCATION_IMPORT_HEADERS` definition with compile-time length assertion, guaranteeing round-trip compatibility and avoiding column drift (unification plan U10).

---

## Key Technical Decisions

- **Follow ProductInventoryImporter pattern exactly:** Two-phase (parse → validate, then transaction upsert), same trait, same helper functions, same ProgressTracker integration. No new infrastructure.
- **Use `ImportSource::Bytes` in the handler:** The handler reads the uploaded file into memory and passes bytes to the importer. This follows the existing pattern and avoids filesystem coupling in tests.
- **RowError as a shared struct, not proto-first:** Define `RowError` in Rust first (`excel_service.rs`), then map into proto response in the handler. This keeps importer logic proto-free and testable.
- **Enum dispatch as a separate upfront unit:** The `ImportType`/`ExportType` enum migration is done first so all subsequent handler changes use the new typed dispatch, avoiding a mix of old string constants and new enum branches in the same file.
- **Soft-delete check in Phase 1, not ON CONFLICT:** ON CONFLICT cannot distinguish between "active record" and "soft-deleted record" without a partial unique index. A query check in Phase 1 is simpler and provides a clear error message.
- **sync_mode as optional flag:** gated behind `optional bool sync_mode` in the proto, defaulting to false. The sync query uses a parameterized location-code list with a 20% safety cap.

---

## Open Questions

### Resolved During Planning

- **Capacity field access:** Location model has `capacity: Option<i32>` — available at the model/repo level. The proto `LocationResponse` does not have capacity, but the Excel import/export works at the service/repo layer, not proto responses, so this is not a problem.

### Deferred to Implementation

- **Exact sqlx query syntax for batch upsert:** Whether to use `INSERT ... ON CONFLICT DO UPDATE` or row-by-row queries depends on whether we pre-load reference data or not. Decide during implementation based on data volume assumptions.
- **CRLF/LF handling in calamine:** If a user's Excel has mixed encoding or special characters in warehouse names, the behavior is unspecified. Handle at implementation time if discovered.
- **R6 scope vs feature delivery:** Whether the enum dispatch refactoring (R6) should remain bundled with this feature or be split into a separate follow-up PR. The plan currently includes it; consider decoupling for lower regression risk. (Scope-guardian review finding)
- **Warning vs error severity for rename detection:** The code rename detection heuristic produces non-blocking warnings, but the RowError struct has no severity field. Add a severity variant to RowError, or collect warnings separately in ImportResult. (Feasibility review finding)

---

## Implementation Units

- U1. **[Proto and model types]** — Add import_type, sync_mode to ImportExcelRequest; add RowError message and row_errors field to ImportResultResponse

**Goal:** Define all proto messages and Rust model types needed by subsequent units.

**Requirements:** R4, R5 (partial R6: enum type definitions)

**Dependencies:** None

**Files:**
- Modify: `proto/abt/v1/excel.proto`
- Modify: `abt/src/service/excel_service.rs`
- Test: (covered by downstream unit tests — proto changes are structural)

**Approach:**
1. In `excel.proto`: Add `string import_type = 3` and `optional bool sync_mode = 4` to `ImportExcelRequest`
2. In `excel.proto`: Add a new `RowError` message with fields `row_index` (uint32), `column_name` (string), `reason` (string), `raw_value` (optional string)
3. In `excel.proto`: Add `repeated RowError row_errors = 4` to `ImportResultResponse`
4. In `excel_service.rs`: Define `ImportType` and `ExportType` enums with `#[non_exhaustive]`, `Display` impls (serialization), and `FromStr` impls (deserialization from proto string fields)
5. In `excel_service.rs`: Define the `RowError` struct with fields: `row_index: usize`, `column_name: String`, `reason: String`, `raw_value: Option<String>`
6. In `excel_service.rs`: Add `row_errors: Vec<RowError>` field to the existing `ImportResult` struct, alongside the existing `errors: Vec<String>` for backward compatibility

**Patterns to follow:**
- Proto message style: existing `ImportResultResponse` in `excel.proto`
- Enum pattern: `WarehouseStatus` / `LocationStatus` in model files — Display impl, serde derives

**Test scenarios:**
- Verify ImportType variants serialize to expected strings (via Display impl)
- Verify ExportType variants serialize to expected strings
- Verify RowError struct creation and field access
- Verify proto round-trip: RowError → proto → RowError

**Verification:**
- `cargo build` succeeds (proto compilation + Rust compilation)
- ImportType/ExportType enums compile with `#[non_exhaustive]`
- RowError struct accessible from both abt crate and abt-grpc crate
- ImportResult now carries row_errors: Vec<RowError>
- ImportType/ExportType round-trip: Display → FromStr yields the same variant

---

- U2. **[WarehouseLocationImporter]** — New Excel import implementation for warehouses and locations

**Goal:** Implement the core import logic: parse Excel, validate rows, upsert warehouses and locations in a transaction.

**Requirements:** R1, R3, R4, R5

**Dependencies:** U1 (RowError struct, ImportType enum), U6 (soft-delete helper)

**Files:**
- Create: `abt/src/implt/excel/warehouse_location_import.rs`
- Test: `abt/src/implt/excel/warehouse_location_import.rs` (module-level tests)

**Approach:**
1. Define `pub const WAREHOUSE_LOCATION_IMPORT_HEADERS: [&str; 5]` with compile-time length assertion in `mod.rs` (shared between importer and exporter, avoiding cross-module dependency)
2. Define private `ExcelRow` struct with serde rename attributes: `仓库编码`, `仓库名称`, `库位编码`, `库位名称`, `容量`
3. Define struct `WarehouseLocationImporter` holding `pool: PgPool` and `tracker: Arc<ProgressTracker>`
4. Implement `ExcelImportService` trait:
   - Phase 1: Parse rows via `RangeDeserializerBuilder`, validate fields, collect into `Vec<ExcelRow>`
     - Check warehouse_name consistency per warehouse_code
     - Detect potential warehouse code renames (name matches but code doesn't)
     - Check for soft-deleted warehouse/location conflicts
   - Phase 1b: Pre-load existing warehouse map (`find_by_codes`) and location map for O(1) lookups in Phase 2.
     - Note: `list_all_with_warehouse` keys by `(warehouse_name, location_code)`, not `(warehouse_code, location_code)`. Add a new query method `list_all_by_warehouse_code` returning locations keyed by `(warehouse_code, location_code)`, or build the mapping in Phase 1b by joining pre-loaded warehouses with locations by `warehouse_id`.
   - Phase 2: Open transaction, iterate rows, for each:
     a. Upsert warehouse by code (find → update or insert), deduplicated via local `HashSet<warehouse_code>` — only the first occurrence per warehouse creates/updates; subsequent rows reuse the confirmed warehouse_id
     b. Upsert location by (warehouse_id, location_code)
     c. Handle soft-delete conflict as blocking error
     d. Tick progress tracker
   - If sync_mode enabled: after upsert loop, soft-delete locations not in file (with 20% safety cap)
   - Collect all errors as `RowError` structs
5. Return `ImportResult` with structured `row_errors`

**Patterns to follow:**
- `product_inventory_import.rs`: two-phase structure, `RangeDeserializerBuilder`, `import_range_from_source()`, `ProgressTracker` usage
- `labor_process_import.rs`: name normalization (if needed for location_name matching)

**Test scenarios:**
- Happy path: Valid file with 3 warehouses and 10 locations — all created
- Upsert: Existing warehouse + location with changed name/capacity — updates applied
- New warehouse: Unknown warehouse_code — warehouse + location both created
- Warehouse name conflict: Same code, different names — blocking error with RowError
- Soft-delete conflict: Code matches deleted record — blocking error reported
- Code rename detection: Code not found but name matches — warning in errors
- Empty file: No rows — zero success, no errors
- Missing required columns: Empty warehouse_code or location_code — RowError per row
- sync_mode: File with subset of locations — extras soft-deleted
- sync_mode safety cap: More than 20% would be deleted — error, no-op
- Invalid capacity: Non-numeric value — RowError with column_name = "容量"
- Progress tracking: tracker.total and tracker.current updated correctly

**Verification:**
- All test scenarios pass
- `cargo test -p abt` passes
- Import produces correct ImportResult with structured RowErrors

---

- U3. **[WarehouseLocationExporter]** — New Excel export for all warehouses and locations

**Goal:** Export all non-deleted warehouses and their locations as a flat Excel file.

**Requirements:** R2

**Dependencies:** None (can be developed in parallel with U2)

**Files:**
- Create: `abt/src/implt/excel/warehouse_location_export.rs`
- Test: `abt/src/implt/excel/warehouse_location_export.rs`

**Approach:**
1. Define and reuse the same `WAREHOUSE_LOCATION_IMPORT_HEADERS` constant for column headers
2. Define struct `WarehouseLocationExporter` holding `pool: PgPool`
3. Implement `ExcelExportService<Params = ()>`:
   - Query: `SELECT w.warehouse_code, w.warehouse_name, l.location_code, l.location_name, l.capacity FROM warehouse w LEFT JOIN location l ON w.warehouse_id = l.warehouse_id AND l.deleted_at IS NULL WHERE w.deleted_at IS NULL ORDER BY w.warehouse_code, l.location_code`
   - Create Workbook, write header row via `write_headers()`
   - Write data rows from query result
   - Return `workbook.save_to_buffer()?`
4. Define a private query result struct matching the SQL columns

**Patterns to follow:**
- `product_without_price_export.rs`: query_as, write_headers, save_to_buffer
- `product_all_export.rs`: similar full-dump export pattern

**Test scenarios:**
- Happy path: Export returns valid Excel bytes
- Empty data: No warehouses — returns Excel with headers only
- Correct columns: Exported Excel has exactly 5 columns matching header constants
- Content accuracy: Exported data matches database state (warehouse/location join)
- Round-trip: Export → Import produces same data (excluding timestamps)

**Verification:**
- Generated .xlsx can be opened by calamine (round-trip test)
- `cargo test -p abt` passes
- Export produces non-empty Excel when data exists

---

- U4. **[Module registration]** — Register new modules and update re-exports

**Goal:** Wire the new importer and exporter into the module tree so they are accessible.

**Requirements:** R1, R2

**Dependencies:** U2, U3

**Files:**
- Modify: `abt/src/implt/excel/mod.rs`

**Approach:**
1. Add `mod warehouse_location_import;` and `mod warehouse_location_export;` to `mod.rs`
2. Add `pub use warehouse_location_import::WarehouseLocationImporter;` and `pub use warehouse_location_export::WarehouseLocationExporter;`
3. Verify the existing `pub use implt::excel;` in `lib.rs` line 30 re-exports these types correctly (no change needed to lib.rs)

**Patterns to follow:**
- Existing import/export module registrations in `mod.rs` (product_inventory_import, labor_process_import, etc.)

**Test scenarios:**
- Test expectation: none — structural change only. Verified by downstream compilation.

**Verification:**
- `cargo build` succeeds
- `WarehouseLocationImporter` and `WarehouseLocationExporter` accessible from abt crate

---

- U5. **[Handler dispatch refactor]** — Add warehouse_location branches and enum-based dispatch

**Goal:** Wire the importer and exporter into the gRPC handler. Replace string-based dispatch with typed enums.

**Requirements:** R1, R2, R4, R5, R6

**Dependencies:** U1 (enums, RowError proto), U2, U3, U4

**Files:**
- Modify: `abt-grpc/src/handlers/excel.rs`
- Test: (integration-level — verified through existing test infrastructure)

**Approach:**
1. Replace `IMPORT_KEY_PRODUCT_INVENTORY` string constant with `ImportType` enum usage
2. Use phased migration strategy: keep existing string constants alongside new enum dispatch, so old clients sending string values continue to work while new clients use typed enums
3. In `import_excel`: 
   - Parse `req.import_type` into `ImportType` enum (with a catch-all for unknown strings → error)
   - Keep backward-compat: empty string defaults to `ImportType::ProductInventory`
   - When `ImportType::WarehouseLocation`: create `WarehouseLocationImporter`, pass `req.sync_mode`, run import, map `RowError` vec into proto `row_errors`
   - Existing `ProductInventory` path continued with backward compatibility
3. In `download_export_file`:
   - Parse `req.export_type` into `ExportType` enum (with catch-all for backward compat)
   - Add `ExportType::WarehouseLocation` branch creating `WarehouseLocationExporter`
4. In `get_progress`:
   - Update key to use `ImportType` enum variants instead of string constants

**Patterns to follow:**
- Existing match dispatch in `download_export_file` — just convert from string match to enum match
- Existing `ProductInventoryImporter` creation pattern in `import_excel`

**Test scenarios:**
- Import with `import_type = "warehouse_location"` triggers WarehouseLocationImporter
- Import with unknown import_type returns error
- Export with `export_type = "warehouse_location"` triggers WarehouseLocationExporter
- Existing import (product_inventory) still works after refactor
- RowError from importer is correctly serialized in proto response
- sync_mode=true flag is passed through to the importer
- Progress tracking key uses ImportType variant (no collision with product_inventory key)

**Verification:**
- `cargo build` succeeds
- `cargo test` passes (existing tests + new unit tests)
- Handler dispatches to correct importer/exporter based on type

---

- U6. **[Soft-delete helper in warehouse_repo]** — Add query method for finding soft-deleted records by code

**Goal:** Support Phase 1 soft-delete conflict detection with a dedicated repository method.

**Requirements:** R3

**Dependencies:** None

**Files:**
- Modify: `abt/src/repositories/warehouse_repo.rs`
- Modify: `abt/src/repositories/location_repo.rs`
- Test: (covered by U2's soft-delete conflict test scenario)

**Approach:**
1. Add method `find_deleted_by_code(pool, code)` to `WarehouseRepo`:
   - Query without `deleted_at IS NULL` filter, explicitly check `deleted_at IS NOT NULL`
   - Return `Option<Warehouse>` only when a soft-deleted match exists
2. Add method `find_deleted_by_code(pool, warehouse_id, location_code)` to `LocationRepo`:
   - Same pattern: query without soft-delete filter, explicitly check `deleted_at IS NOT NULL`
   - Required for U2's soft-delete conflict detection at the location level
3. Add method `find_by_codes(pool, codes)` to `WarehouseRepo`:
   - Accept `&[String]` or similar, return `Vec<Warehouse>`
   - Required by U2 Phase 1b for pre-loading warehouse map in bulk

**Patterns to follow:**
- Existing `find_by_code` in `warehouse_repo.rs` — same query but without `deleted_at IS NULL`
- Existing `find_by_codes` in `product_repo.rs` — pattern for batch lookup by code list

**Test scenarios:**
- Test expectation: none — covered by U2's integration test scenarios

**Verification:**
- `cargo build` succeeds
- Method returns soft-deleted record correctly

---

## System-Wide Impact

- **Interaction graph:** The existing `AbtExcelService` dispatch grows two new branches. No changes to warehouse/location CRUD paths.
- **Error propagation:** Phase 1 validation errors use `business_error()` (no log noise). Phase 2 DB errors use `err_to_status()` (logged). Proto `RowError` is a new error surface in the `ImportResultResponse`.
- **Transaction atomicity vs partial-success**: Resolve by using per-row PostgreSQL SAVEPOINTs within the transaction — a DB error on one row rolls back only that row's changes via ROLLBACK TO SAVEPOINT, while the transaction continues. This allows collecting per-row RowErrors alongside partial success, unlike a single all-or-nothing transaction. (Adversarial review finding)
- **API surface parity:** No new gRPC methods. Proto changes are additive (new fields on existing messages, new message type). Existing clients sending `ImportExcelRequest` without `import_type` field will get empty string → we need backward-compat handling (empty string = default to product_inventory for legacy clients).
- **Unchanged invariants:** All existing warehouse/location CRUD RPCs and their proto messages are untouched. All existing import/export flows (product_inventory, labor_process, etc.) continue working identically.
- **Integration coverage:** The import unit tests prove the full parse → validate → upsert → error-collect pipeline. The export unit tests prove the query → write → buffer pipeline. Handler tests prove dispatch routing only — the actual import/export logic is tested at the service layer.

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| **Backward compat: old clients send ImportExcelRequest without import_type** | Default empty string to `ImportType::ProductInventory` for legacy support |
| **Proto field number conflict: RowError field 4 on ImportResultResponse** | Field 3 on ImportResultResponse is `repeated string errors`. Field 4 is unused — safe to use. Verify before committing |
| **sync_mode safety cap denominator undefined** | Define as per-warehouse percentage: `deletion_count / pre_sync_location_count * 100` per unique warehouse_code. If any warehouse exceeds 20%, reject the entire sync_mode operation with an error listing offending warehouses. Apply an absolute minimum floor (at least 1 location) so small warehouses are not silently wiped. (Adversarial review finding) |
| **TOCTOU race: concurrent CRUD between Phase 1b pre-load and Phase 2 upsert** | Mitigate by moving pre-load queries inside the Phase 2 transaction, or using `INSERT ... ON CONFLICT DO UPDATE` with a soft-delete filter to handle races without separate find-then-insert. (Adversarial review finding) |
| **Capacity column type mismatch in calamine** | Use `deserialize_optional_decimal` pattern or direct string parsing in ExcelRow. Handle type coercion gracefully |

---

## Documentation / Operational Notes

- After deployment, clients need to know the new `import_type = "warehouse_location"` value and the `sync_mode` option. Document in the API reference.
- The export type string `"warehouse_location"` works with the `DownloadExportFile` (streaming) RPC.
