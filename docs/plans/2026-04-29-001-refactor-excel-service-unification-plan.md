---
title: "refactor: Unify Excel import/export into shared trait abstraction"
type: refactor
status: complete
date: 2026-04-29
origin: docs/superpowers/specs/2026-04-29-excel-service-unification-design.md
ideation: docs/ideation/2026-04-29-excel-service-unification-ideation.md
---

# refactor: Unify Excel import/export into shared trait abstraction

## Overview

Extract scattered Excel import/export logic from 4 domain service traits into two unified traits (`ExcelImportService`, `ExcelExportService`) with per-operation light-weight structs, `Arc<ProgressTracker>` progress tracking, `ImportSource` bytes abstraction for testability, and registry-based handler dispatch.

The `OnceLock<ProductExcelServiceImpl>` singleton — the last remaining singleton in the codebase — is eliminated. Every Excel operation becomes a stateless or request-scoped struct following the same `Arc<Pool>` DI pattern as all 17 other services.

---

## Problem Frame

Currently, Excel import/export logic is embedded in 4 different service traits (ProductExcelService, LaborProcessService, LaborProcessDictService, BomService) with no shared abstraction. The product Excel service uses a `OnceLock` global singleton — the only service in the codebase to do so — solely to maintain AtomicUsize progress counters across gRPC requests. Path-security validation is duplicated in two places. Export-to-bytes and export-to-file are nearly identical copy-paste in product and BOM implementations. The gRPC handler dispatches exports via a bare string match with no compile-time safety.

The unification creates two fine-grained traits, moves progress tracking out of business logic into a standalone `ProgressTracker`, accepts `ImportSource` (bytes or path) for testability, and replaces string-based dispatch with a typed registry.

---

## Requirements Trace

- R1. Define `ExcelImportService` and `ExcelExportService` traits with clean, minimal signatures
- R2. Create one lightweight struct per import/export operation, each implementing exactly one trait
- R3. Replace `OnceLock<ProductExcelServiceImpl>` singleton with per-request `Arc<ProgressTracker>`
- R4. Use `ImportSource` enum (`Path` / `Bytes`) to decouple import logic from filesystem
- R5. Accept request-scoped parameters via method parameter rather than constructor binding
- R6. Extract all Excel logic from `LaborProcessService`, `LaborProcessDictService`, `BomService` traits
- R7. Update all 4 gRPC handlers to route through new implementations
- R8. Add schema-as-code column constants to guarantee import/export round-trip consistency
- R9. Add registry-based handler dispatch so new exports don't require handler changes
- R10. Maintain backward compatibility during migration via forwarding shims

**Origin actors:** N/A (internal refactor, no user-facing behavior change)
**Origin flows:** N/A

---

## Scope Boundaries

- Delete `abt/src/service/product_excel_service.rs` and `abt/src/implt/product_excel_service_impl.rs` after migration
- gRPC proto definitions are **not** changed in this round; string-based `export_type` is wrapped in a Rust enum at the handler boundary
- BOM export formatting (cell styles, row heights, column widths) is preserved as-is in the extracted `BomExporter`
- Permission annotations on gRPC handler methods are preserved unchanged
- The `stream_excel_bytes` utility in `abt-grpc/src/handlers/mod.rs` is reused as-is

### Deferred to Follow-Up Work

- Proto enum for `export_type` (replacing bare string in `DownloadExportFileRequest`): separate proto-change PR
- Streaming progress response (replacing poll-based `GetProgress`): requires proto change
- Dry-run/preview import mode: separate feature
- Pull-based row-by-row import processing: optimization, not needed for correctness
- Location Excel import/export: will follow this pattern once the infrastructure is in place

---

## Context & Research

### Relevant Code and Patterns

- **Factory function pattern**: All 17 non-Excel services use `Arc::new(ctx.pool().clone())` in `abt/src/lib.rs` lines 122-214
- **Service trait pattern**: `#[async_trait] pub trait XxxService: Send + Sync` in `abt/src/service/`
- **Handler pattern**: `#[tonic::async_trait] impl GrpcXxxService for XxxHandler` with `AppState::get().await` in `abt-grpc/src/handlers/`
- **Excel read**: calamine `open_workbook(path)` → `worksheet_range_at(0)` → `RangeDeserializerBuilder::with_headers()` in `product_excel_service_impl.rs:97-112`
- **Excel write**: rust_xlsxwriter `Workbook::new()` → `add_worksheet()` → `write_string/write_number` → `save_to_buffer()` in all export methods
- **Transaction pattern**: `pool.begin().await` → batch operations → `tx.commit().await` in product and labor process imports
- **Streaming response**: `stream_excel_bytes(file_name, bytes)` shared utility in `abt-grpc/src/handlers/mod.rs:63-107`
- **Permission macro**: `#[require_permission(Resource::Xxx, Action::Yyy)]` on gRPC handler methods

### Institutional Learnings

- **OnceLock audit warning** (`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`): Soft-init failures in OnceLock become permanent. Progress state should be per-operation, not global.
- **Incremental migration pattern** (`docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`): Coexist old and new implementations with forwarding shims, update handlers one by one, then remove shims.
- **Service trait refactoring** (`docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`): Use query/request structs for stable method signatures during refactoring.
- **BomServiceImpl OnceLock<Format>**: A separate `OnceLock<Format>` exists for cell formatting (read-only after init, `bom_service_impl.rs:25,39,52,63`). This is safe (lazy-init formatter, no mutable state) and is **not** affected by this refactor.

### External References

- vantage-dataset crate: Capability-separated Read/Write traits pattern (import ≠ export, different capabilities)
- bdk/gifski/dircat: `NoopReporter` null-object pattern for progress tracking
- sitrep crate: Channel-based progress with producer/consumer split
- calamine supports `Xlsx::new(BufReader<Cursor<Vec<u8>>>)` for in-memory XLSX reading

---

## Key Technical Decisions

- **Import/export split into two traits**: They are different capabilities with different error modes and data flows. Industry consensus from vantage-dataset, Pandoc, and std::io Read/Write separation.
- **ProgressTracker as standalone struct, not trait method**: 5 cross-domain analogies independently converged on "progress is infrastructure, not business logic." Eliminates dual-source-of-truth between trait method and factory-returned Arc.
- **`ImportSource` enum (`Path` / `Bytes`)**: Single filesystem boundary at handler layer. Enables pure in-memory testing. calamine supports `Cursor<Vec<u8>>` for XLSX.
- **Export request via method parameter, not constructor**: `BomExporter` needs `bom_id` from the gRPC request. Constructor binding forces per-request re-construction for no benefit. Method parameter keeps the state/behavior separation clean.
- **Rust enum wrapping proto string**: Proto `export_type: string` is wrapped in a Rust enum at the handler boundary for compile-time exhaustiveness, deferring the proto change to a follow-up PR.
- **Registry at handler init, not compile-time macro**: A `HashMap` populated at handler construction is simpler than a proc-macro registry, and the handler already follows the `new()` pattern.

---

## Output Structure

```
abt/src/
  service/
    excel_service.rs                      # NEW: traits + types
    product_excel_service.rs              # DELETED (Phase 4)
  implt/
    excel/
      mod.rs                              # NEW
      progress.rs                         # NEW: ProgressTracker
      product_inventory_import.rs         # NEW
      product_all_export.rs               # NEW
      product_without_price_export.rs     # NEW
      labor_process_import.rs             # NEW
      labor_process_export.rs             # NEW
      labor_process_dict_export.rs        # NEW
      boms_no_labor_cost_export.rs        # NEW
      bom_export.rs                       # NEW
    product_excel_service_impl.rs         # DELETED (Phase 4)

abt-grpc/src/handlers/
  excel.rs                                # MODIFIED: uses new traits + registry
  labor_process.rs                        # MODIFIED: uses Excel structs
  labor_process_dict.rs                   # MODIFIED: uses Excel struct
  bom.rs                                  # MODIFIED: uses Excel struct
```

---

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

### Trait Definitions

```rust
// abt/src/service/excel_service.rs

pub struct ImportResult { success_count: usize, failed_count: usize, errors: Vec<String> }
pub struct ExcelProgress { current: usize, total: usize }

pub enum ImportSource { Path(PathBuf), Bytes(Vec<u8>) }

pub struct ExportRequest<T> { pub params: T }

#[async_trait]
pub trait ExcelImportService: Send + Sync {
    async fn import(&self, source: ImportSource) -> Result<ImportResult>;
}

#[async_trait]
pub trait ExcelExportService: Send + Sync {
    type Params: Send + Sync;
    async fn export(&self, req: ExportRequest<Self::Params>) -> Result<Vec<u8>>;
}
```

### ProgressTracker (standalone, not on trait)

```rust
// abt/src/implt/excel/progress.rs

pub struct ProgressTracker { current: AtomicUsize, total: AtomicUsize }
impl ProgressTracker {
    pub fn new() -> Arc<Self> { ... }
    pub fn set_total(&self, n: usize) { ... }
    pub fn tick(&self) { ... }
    pub fn snapshot(&self) -> ExcelProgress { ... }
}
```

### Handler Progress Storage

```rust
// In ExcelHandler
active_imports: Mutex<HashMap<String, Arc<ProgressTracker>>>
// key = import_type string; populated before import, cleared after completion
```

### Export Registry Dispatch

```rust
type ExportFn = Arc<dyn Fn(&PgPool, Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>> + Send + Sync>;

// In ExcelHandler
export_registry: HashMap<ExportType, ExportFn>
// populated at handler construction; download_export_file looks up by type
```

---

## Implementation Units

### Phase 1 — Core Infrastructure

- [x] U1. **Define `ExcelImportService` and `ExcelExportService` traits with shared types**

**Goal:** Create the new trait file with `ImportResult`, `ExcelProgress`, `ImportSource`, `ExportRequest`, and both traits.

**Requirements:** R1, R4, R5

**Dependencies:** None

**Files:**
- Create: `abt/src/service/excel_service.rs`
- Modify: `abt/src/service/mod.rs` (add `mod excel_service; pub use excel_service::*;`)

**Approach:**
- Move `ImportResult` and `ExcelProgress` from `product_excel_service.rs` into `excel_service.rs` as shared types
- `ImportSource` enum: `Path(PathBuf)` and `Bytes(Vec<u8>)` variants
- `ExportRequest<T>` generic wrapper for request-scoped parameters
- `ExcelImportService`: single method `async fn import(&self, source: ImportSource) -> Result<ImportResult>`
- `ExcelExportService`: associated type `Params` + `async fn export(&self, req: ExportRequest<Self::Params>) -> Result<Vec<u8>>`
- Keep `product_excel_service.rs` as-is for now (it still defines `ProductExcelService` trait); add `pub use` re-exports so existing code compiles

**Patterns to follow:**
- Existing trait style in `abt/src/service/product_excel_service.rs` (Send + Sync, #[async_trait], anyhow::Result)
- `ExportRequest<T>` pattern from `models/labor_process.rs` query struct approach

**Test scenarios:**
- Happy path: Trait compiles with Send + Sync bounds satisfied
- Happy path: `ImportSource::Path` and `ImportSource::Bytes` variants construct correctly
- Edge case: `ExportRequest` with unit type `()` for parameterless exports (e.g., dict export)

**Verification:**
- `cargo build -p abt` compiles successfully with the new module
- Existing tests continue to pass (types are additive, no behavior change yet)

---

- [x] U2. **Create `ProgressTracker` and `implt/excel/` module structure**

**Goal:** Create the standalone progress tracker and the directory scaffold for per-operation implementations.

**Requirements:** R3

**Dependencies:** U1

**Files:**
- Create: `abt/src/implt/excel/mod.rs`
- Create: `abt/src/implt/excel/progress.rs`
- Modify: `abt/src/implt/mod.rs` (add `mod excel;`)

**Approach:**
- `ProgressTracker`: wraps `AtomicUsize` for current and total, provides `new()` → `Arc<Self>`, `set_total()`, `tick()`, `snapshot()` → `ExcelProgress`
- `Arc<ProgressTracker>` is clonable and can be held by both the importer (to update) and the handler (to query via GetProgress)
- `implt/excel/mod.rs` initially re-exports only `ProgressTracker`; per-operation structs are added in subsequent units

**Patterns to follow:**
- `AtomicUsize` with `Ordering::SeqCst` from existing `ProductExcelServiceImpl` (lines 79-80 of impl)
- Newtype module pattern from `implt/mod.rs` (flat `pub use` re-exports)

**Test scenarios:**
- Happy path: `ProgressTracker::new()` returns tracker with current=0, total=0
- Happy path: `set_total(100)` then `tick()` 5 times → `snapshot()` returns `ExcelProgress { current: 5, total: 100 }`
- Edge case: Multiple concurrent `tick()` calls from different threads maintain correct count (within AtomicUsize precision)
- Edge case: `snapshot()` on fresh tracker returns `ExcelProgress { current: 0, total: 0 }`

**Verification:**
- `cargo build -p abt` compiles
- Unit test: create tracker, set total, tick N times, verify snapshot

---

### Phase 2 — Product Excel Migration

- [x] U3. **Extract product import/export implementations**

**Goal:** Create `ProductInventoryImporter`, `ProductAllExporter`, `ProductWithoutPriceExporter` structs by extracting logic from `product_excel_service_impl.rs`.

**Requirements:** R2, R4, R5

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/implt/excel/product_inventory_import.rs`
- Create: `abt/src/implt/excel/product_all_export.rs`
- Create: `abt/src/implt/excel/product_without_price_export.rs`
- Modify: `abt/src/implt/excel/mod.rs` (add re-exports)

**Approach:**
- `ProductInventoryImporter`: holds `PgPool`, `Arc<ProgressTracker>`, and optional `operator_id`. `import(source: ImportSource)` — when source is Bytes, use `Cursor<Vec<u8>>` with `Xlsx::new(BufReader::new(cursor))`; when Path, use existing `open_workbook`. Move `ExcelRow`, `PendingItem`, `deserialize_empty_decimal`, `update_price_batch`, `upsert_inventory_quantity`, `upsert_inventory_safety_stock` into this file as module-private helpers
- `ProductAllExporter`: holds `PgPool`. `Params = ()`. Uses `InventoryRepo::list_for_export`
- `ProductWithoutPriceExporter`: holds `PgPool`. `Params = ()`. Uses inline SQL for products without price. Export column headers defined as `const HEADERS: [&str; 8]`
- Define shared column header constants as `pub const PRODUCT_IMPORT_HEADERS: [&str; 8]` for import and `pub const PRODUCT_EXPORT_HEADERS: [&str; 10]` for full export — schema-as-code foundation (R8)
- Do NOT delete `product_excel_service_impl.rs` yet — keep it as forwarding shim

**Patterns to follow:**
- calamine reader pattern from `product_excel_service_impl.rs:97-112`
- rust_xlsxwriter export pattern from `product_excel_service_impl.rs:263-327`
- Transaction pattern from `product_excel_service_impl.rs:215-257`

**Test scenarios:**
- Happy path: `ProductInventoryImporter` with `ImportSource::Bytes(valid_xlsx_bytes)` imports inventory correctly
- Happy path: `ProductAllExporter` exports all products to bytes with expected headers
- Happy path: `ProductWithoutPriceExporter` exports only products with null/zero price
- Edge case: Import with empty XLSX (only headers) returns `ImportResult { success_count: 0, failed_count: 0 }`
- Error path: Import with missing required column produces error in `ImportResult.errors`
- Error path: Import with invalid product code produces per-row error message
- Integration: Export → re-import round-trip: export products, modify price column, import back, verify price updated

**Verification:**
- `cargo build -p abt` compiles
- New structs implement correct traits
- Existing `ProductExcelServiceImpl` still works as forwarding shim (all existing tests pass)

---

- [x] U4. **Replace OnceLock singleton with factory functions returning new-style structs**

**Goal:** Update `lib.rs` to remove `OnceLock<EXCEL_SERVICE>` and add factory functions for the three new product Excel structs.

**Requirements:** R3

**Dependencies:** U3

**Files:**
- Modify: `abt/src/lib.rs` (remove lines 70, 157-159, 221-247; add new factory functions)

**Approach:**
- Remove `static EXCEL_SERVICE: OnceLock<implt::ProductExcelServiceImpl> = OnceLock::new();` (line 70)
- Replace `get_product_excel_service()` with:
  - `pub fn get_product_inventory_importer(pool: &PgPool) -> (impl ExcelImportService, Arc<ProgressTracker>)`
  - `pub fn get_product_all_exporter(pool: &PgPool) -> impl ExcelExportService`
  - `pub fn get_product_without_price_exporter(pool: &PgPool) -> impl ExcelExportService`
- Remove the `impl ProductExcelService for &S` blanket impl (lines 221-247) — no longer needed since no singleton reference
- Keep `pub use service::ProductExcelService;` re-export temporarily until Phase 4 cleanup

**Patterns to follow:**
- Standard factory pattern from `lib.rs:122-214`: `Arc::new(ctx.pool().clone())` constructor
- Return `impl Trait` from factory (consistent with all other factories)

**Test scenarios:**
- Happy path: Factory returns struct implementing correct trait
- Happy path: `ProgressTracker` from factory is independently clonable and readable
- Edge case: Two concurrent factories return independent importers with independent progress trackers

**Verification:**
- `cargo build -p abt` compiles without `OnceLock<EXCEL_SERVICE>`
- `cargo build -p abt-grpc` compiles (handler updated in U5)
- No `OnceLock` usage remains related to Excel service

---

- [x] U5. **Update `AbtExcelService` gRPC handler to use new traits**

**Goal:** Replace `ProductExcelService` trait usage in `excel.rs` with `ExcelImportService`/`ExcelExportService` and `Arc<ProgressTracker>`.

**Requirements:** R3, R7

**Dependencies:** U4

**Files:**
- Modify: `abt-grpc/src/handlers/excel.rs`
- Modify: `abt-grpc/src/server.rs` (update `excel_service()` accessor if needed)

**Approach:**
- Replace `use abt::ProductExcelService;` with `use abt::{ExcelImportService, ExcelExportService};`
- `import_excel`: Create `ProductInventoryImporter` via factory, get `Arc<ProgressTracker>`. Store tracker in `active_imports: Mutex<HashMap<String, Arc<ProgressTracker>>>` keyed by `"product_inventory"`. Call `importer.import(source)`. On completion (success or error), remove from map
- `get_progress`: Look up `"product_inventory"` in `active_imports`, call `tracker.snapshot()`. If no active import, return `ExcelProgress::default()`
- `download_export_file`: Use `match` on `req.export_type` with a Rust enum (`ExportType`) that wraps the proto string. Each arm constructs the appropriate exporter and calls `export()`. Default arm returns `invalid_argument` error with list of valid types
- File upload path validation stays in handler only (no longer duplicated in service layer, since import now accepts `ImportSource::Bytes`)
- `export_excel`: Construct `ProductAllExporter`, call export, write bytes to file path (one-off filesystem operation in handler)
- Update `AppState::excel_service()` in `server.rs` if needed — may no longer need a single accessor

**Patterns to follow:**
- Existing handler pattern in `excel.rs` (permission annotations, `AppState::get().await`, `stream_excel_bytes`)
- `ExportType` enum with `FromStr` impl wrapping the proto string
- `HashMap<String, Arc<ProgressTracker>>` behind `Mutex` for active import tracking

**Test scenarios:**
- Happy path: `import_excel` with valid bytes returns correct `ImportResultResponse`
- Happy path: `get_progress` during active import returns current/total > 0
- Happy path: `get_progress` with no active import returns `{ current: 0, total: 0 }`
- Happy path: `download_export_file` with `"products"` returns streaming product data
- Happy path: `download_export_file` with `"products_without_price"` returns streaming without-price data
- Error path: `download_export_file` with unknown export type returns `invalid_argument`
- Error path: `import_excel` with non-XLSX bytes returns error
- Integration: Upload file → import → get_progress → download_export_file flow works end-to-end

**Verification:**
- `cargo build -p abt-grpc` compiles
- Server starts and all Excel RPCs respond correctly
- Permission checks (`Resource::Excel`, `Action::Read`/`Write`) still enforced

---

### Phase 3 — Domain Service Excel Extraction

- [x] U6. **Extract labor process Excel import/export implementations**

**Goal:** Move Excel import/export logic from `labor_process_service_impl.rs` into dedicated `LaborProcessImporter` and `LaborProcessExporter` structs.

**Requirements:** R2, R4, R6

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/implt/excel/labor_process_import.rs`
- Create: `abt/src/implt/excel/labor_process_export.rs`
- Modify: `abt/src/implt/excel/mod.rs`
- Modify: `abt/src/implt/labor_process_service_impl.rs` (add forwarding shims, don't remove yet)

**Approach:**
- `LaborProcessImporter`: holds `PgPool`, `Arc<ProgressTracker>`, and `routing_service: Arc<dyn RoutingService>`. Uses `ImportSource`. Moves `LABOR_PROCESS_EXCEL_COLUMNS`, `ExcelRow`, `ValidLaborProcessRow`, `normalize_process_name`, `unique_sorted_process_codes`, `validate_process_codes`, `auto_route`, `row_error` from `labor_process_service_impl.rs` and `models/labor_process.rs`. Column constants become `pub const LABOR_PROCESS_IMPORT_COLUMNS: [&str; 7]`
- `LaborProcessExporter`: holds `PgPool`. `Params = String` (product_code). Uses `LaborProcessRepo::list_all_by_product_code`
- Keep original methods in `LaborProcessServiceImpl` as forwarding shims calling the new structs
- Move `LaborProcessImportResult` → converted to `ImportResult` (routing_results discarded per design spec; errors converted to strings)

**Patterns to follow:**
- calamine reader pattern (identical to product import)
- Validation pattern from `labor_process_service_impl.rs:91-212` (parse → validate → check duplicates → check process codes)
- Transaction pattern from `labor_process_service_impl.rs:346-401`

**Test scenarios:**
- Happy path: Import valid labor process XLSX with existing product codes and process codes
- Edge case: Import with duplicate process names within same product → error in result
- Edge case: Import with process codes not in dict → validation error
- Error path: Import with empty product_code column → row error
- Error path: Import with negative unit price → row error
- Happy path: Export by product_code returns correct XLSX bytes with expected columns
- Integration: Import creates auto-routing entries for products without existing routes

**Verification:**
- `cargo build -p abt` compiles
- Existing labor process handler continues to work via forwarding shims
- Column constants match between import and export for round-trip consistency

---

- [x] U7. **Extract labor process dict and BOM Excel export implementations**

**Goal:** Create `LaborProcessDictExporter`, `BomsWithoutLaborCostExporter`, and `BomExporter` structs.

**Requirements:** R2, R6

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/implt/excel/labor_process_dict_export.rs`
- Create: `abt/src/implt/excel/boms_no_labor_cost_export.rs`
- Create: `abt/src/implt/excel/bom_export.rs`
- Modify: `abt/src/implt/excel/mod.rs`
- Modify: `abt/src/implt/labor_process_dict_service_impl.rs` (forwarding shim)
- Modify: `abt/src/implt/labor_process_service_impl.rs` (forwarding shim for `export_boms_without_labor_cost`)
- Modify: `abt/src/implt/bom_service_impl.rs` (forwarding shim)

**Approach:**
- `LaborProcessDictExporter`: holds `PgPool`. `Params = ()`. Uses `LaborProcessDictRepo::list_all`. Column constants: `pub const DICT_EXPORT_COLUMNS: [&str; 4]`
- `BomsWithoutLaborCostExporter`: holds `PgPool`. `Params = ()`. Uses `LaborProcessRepo::find_boms_without_labor_cost`. Column constants: `pub const BOMS_NO_COST_COLUMNS: [&str; 4]`
- `BomExporter`: holds `PgPool`. `Params = i64` (bom_id). Uses `BomRepo::find_by_id_pool` + `ProductRepo::find_by_ids`. Preserves existing cell formatting logic (`header_format`, `top_level_format`, parent/node formatting) from `bom_service_impl.rs`. Export returns `(Vec<u8>, String)` where String is BOM name — adapt to `ExportRequest` pattern by returning BOM name in a wrapper or via separate query
- All three keep forwarding shims in original service impls

**Patterns to follow:**
- BomExporter: formatting helpers from `bom_service_impl.rs:82-170` (cell styles, row heights, column widths)
- Dict exporter: simple 20-line pattern from `labor_process_dict_service_impl.rs:102-122`

**Test scenarios:**
- Happy path: `LaborProcessDictExporter` exports all dict entries with expected 4 columns
- Happy path: `BomsWithoutLaborCostExporter` exports BOMs missing labor cost with expected columns
- Happy path: `BomExporter` exports BOM tree with formatted cells
- Edge case: Dict export with empty database → XLSX with headers only
- Edge case: BOM export with no children → single-row XLSX

**Verification:**
- `cargo build -p abt` compiles
- All three structs implement `ExcelExportService`
- Forwarding shims in original services produce identical output

---

- [x] U8. **Remove Excel methods from domain service traits and update gRPC handlers**

**Goal:** Strip Excel methods from `LaborProcessService`, `LaborProcessDictService`, `BomService` traits and their impls. Update corresponding gRPC handlers to use new Excel structs directly.

**Requirements:** R6, R7

**Dependencies:** U6, U7

**Files:**
- Modify: `abt/src/service/labor_process_service.rs` (remove `import_from_excel`, `export_to_bytes`, `export_boms_without_labor_cost`)
- Modify: `abt/src/service/labor_process_dict_service.rs` (remove `export_to_bytes`)
- Modify: `abt/src/service/bom_service.rs` (remove `export_to_excel`, `export_to_bytes`)
- Modify: `abt/src/implt/labor_process_service_impl.rs` (remove Excel methods and helpers)
- Modify: `abt/src/implt/labor_process_dict_service_impl.rs` (remove `export_to_bytes`)
- Modify: `abt/src/implt/bom_service_impl.rs` (remove `export_to_excel`, `export_to_bytes`, `build_export_workbook`)
- Modify: `abt-grpc/src/handlers/labor_process.rs` (use `LaborProcessImporter`, `LaborProcessExporter`, `BomsWithoutLaborCostExporter`)
- Modify: `abt-grpc/src/handlers/labor_process_dict.rs` (use `LaborProcessDictExporter`)
- Modify: `abt-grpc/src/handlers/bom.rs` (use `BomExporter`)
- Modify: `abt-grpc/src/server.rs` (add accessor methods or inline factory calls for new Excel structs)

**Approach:**
- Labor process handler `import_labor_processes`: Construct `LaborProcessImporter`, pass routing_service. Use `ImportSource::Path` (existing behavior, no proto change). Store progress tracker for GetProgress if needed
- Labor process handler `export_labor_processes`: Construct `LaborProcessExporter` with `req.product_code`, call `export(ExportRequest { params: product_code })`
- BOM handler `download_bom`: Construct `BomExporter`, call `export(ExportRequest { params: bom_id })`, stream result bytes with BOM name as file name
- Remove `build_export_workbook` and all Excel helper functions from `bom_service_impl.rs`
- Remove `ExcelRow`, `ValidLaborProcessRow`, `normalize_process_name`, `unique_sorted_process_codes`, `validate_process_codes`, `auto_route`, `row_error` from `labor_process_service_impl.rs` and `models/labor_process.rs`

**Patterns to follow:**
- Handler construction pattern: `let importer = abt::get_labor_process_importer(&state.pool());`
- Streaming response: reuse `stream_excel_bytes(file_name, bytes)` unchanged

**Test scenarios:**
- Integration: Labor process import via gRPC → correct ImportResultResponse
- Integration: Labor process export via gRPC → correct streaming XLSX
- Integration: BOM download via gRPC → correct streaming XLSX with formatting
- Integration: Dict export via gRPC → correct streaming XLSX
- Happy path: BomService trait no longer has `export_to_excel` or `export_to_bytes` methods
- Happy path: LaborProcessService trait no longer has Excel methods

**Verification:**
- `cargo build -p abt -p abt-grpc` compiles
- `cargo test` passes all existing tests
- Server starts and all 4 handlers' Excel RPCs respond correctly

---

### Phase 4 — Cleanup & Hardening

- [x] U9. **Delete old files and finalize module structure**

**Goal:** Remove `product_excel_service.rs`, `product_excel_service_impl.rs`, and clean up re-exports.

**Requirements:** R1

**Dependencies:** U8

**Files:**
- Delete: `abt/src/service/product_excel_service.rs`
- Delete: `abt/src/implt/product_excel_service_impl.rs`
- Modify: `abt/src/service/mod.rs` (remove `mod product_excel_service` and its `pub use`)
- Modify: `abt/src/implt/mod.rs` (remove `mod product_excel_service_impl` and its `pub use`)
- Modify: `abt/src/lib.rs` (remove `pub use service::ProductExcelService;` re-export if still present)

**Approach:**
- Verify no remaining references to `ProductExcelService` or `ProductExcelServiceImpl` via `cargo check`
- Delete both files
- Remove module declarations and re-exports
- Update `lib.rs` to remove any remaining `ProductExcelService`-related code

**Test scenarios:**
- Test expectation: none — pure deletion, verified by compilation

**Verification:**
- `cargo build -p abt -p abt-grpc` compiles with zero references to deleted types
- `cargo test` passes
- `rg "ProductExcelService\|ProductExcelServiceImpl" abt/src/ abt-grpc/src/` returns no results

---

- [x] U10. **Add schema-as-code column constants and round-trip consistency**

**Goal:** Ensure every Excel operation defines its column schema as a `const`, and import/export for the same data type share the same column definition.

**Requirements:** R8

**Dependencies:** U9

**Files:**
- Modify: `abt/src/implt/excel/product_inventory_import.rs` (add `pub const` for headers)
- Modify: `abt/src/implt/excel/product_without_price_export.rs` (reference shared `PRODUCT_IMPORT_COLUMNS`)
- Modify: `abt/src/implt/excel/labor_process_import.rs` (add `pub const LABOR_PROCESS_COLUMNS`)
- Modify: `abt/src/implt/excel/labor_process_export.rs` (reference shared constant)
- Modify: `abt/src/implt/excel/labor_process_dict_export.rs` (add `pub const DICT_COLUMNS`)
- Modify: `abt/src/implt/excel/boms_no_labor_cost_export.rs` (add `pub const BOMS_NO_COST_COLUMNS`)
- Modify: `abt/src/implt/excel/bom_export.rs` (add `pub const BOM_EXPORT_COLUMNS`)

**Approach:**
- Each importer/exporter pair defines shared column constants as `pub const`
- Import uses the constant in `RangeDeserializerBuilder::with_headers(&CONSTANT)`
- Export uses the same constant when writing header row
- Add a compile-time or test-time assertion where possible (e.g., `const _: () = assert!(PRODUCT_IMPORT_COLUMNS.len() == 8);`)

**Test scenarios:**
- Happy path: Product import and "without price" export use identical header constants
- Happy path: Labor process import and export use identical header constants
- Edge case: Changing a constant affects both import and export at compile time

**Verification:**
- `cargo build` compiles
- All header constants are `pub const` and referenced by both import and export for the same data type

---

- [x] U11. **Add registry-based dispatch to Excel gRPC handler**

**Goal:** Replace the `match export_type` in `download_export_file` with a `HashMap<ExportType, ExportFn>` registry populated at handler construction.

**Requirements:** R9

**Dependencies:** U8

**Files:**
- Modify: `abt-grpc/src/handlers/excel.rs`

**Approach:**
- Define `ExportType` enum wrapping the proto string variants: `Products`, `ProductsWithoutPrice`, `Bom`, `LaborProcess`, `LaborProcessDict`
- Define `ExportFn` type alias: `Arc<dyn Fn(&PgPool, Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>> + Send + Sync>` — or a simpler trait-object approach using `Box<dyn ExcelExportService>`
- Simpler approach: registry maps `ExportType` to a factory closure that returns `Box<dyn ExcelExportService<Params = SomeConcreteType>>` — but associated types make this tricky. Use an enum-based approach instead:
  - `ExportRegistry` is a `HashMap<ExportType, Box<dyn Fn(&PgPool, &[u8]) -> Pin<Box<dyn Future<Output = Result<(Vec<u8>, String)>> + Send>> + Send + Sync>>` where the closure captures the export type and the `&[u8]` is serialized params
- Populate registry in `ExcelHandler::new()` (or a `Default` impl)
- `download_export_file`: parse `export_type` into `ExportType`, lookup registry, call closure with pool + params bytes
- Unknown type returns `invalid_argument` with list of registered types

**Patterns to follow:**
- Handler `new()` pattern from `excel.rs:19-23`
- Enum wrapping pattern from `permissions/mod.rs` (Resource, Action enums)

**Test scenarios:**
- Happy path: All registered export types produce valid responses
- Edge case: Adding a new `ExportType` variant and registering it does not require changing `download_export_file` method body
- Error path: Requesting unregistered export type returns `invalid_argument` with available types listed

**Verification:**
- `cargo build -p abt-grpc` compiles
- All existing export types work through registry
- Compiler enforces exhaustiveness of `ExportType` enum

---

## System-Wide Impact

- **Interaction graph:** 4 gRPC handlers (excel, labor_process, labor_process_dict, bom) touch Excel operations. All now route through `implt/excel/` structs. The `AppState` accessor for `excel_service()` may be simplified or removed
- **Error propagation:** `ImportResult.errors: Vec<String>` is the unified error carrier for all import types. Per-row labor process errors (previously `LaborProcessImportRowResult`) are flattened to strings. Handler-level error conversion (`err_to_status`, `business_error`, `validation`) is unchanged
- **State lifecycle risks:** `Mutex<HashMap<String, Arc<ProgressTracker>>>` in handler: stale entries if import panics. Mitigation: `Drop` impl or `catch_unwind` in import spawn to clean up. Import timeout (future concern, not in this plan)
- **API surface parity:** All 4 proto services keep their existing RPC signatures unchanged. Response types are unchanged. Client-visible behavior is preserved
- **Integration coverage:** Upload→Import→GetProgress flow; Export→Download flow across all 4 handlers. BOM export formatting regression test
- **Unchanged invariants:** Permission annotations preserved. `stream_excel_bytes` utility unchanged. Database schema unchanged. `AbtExcelService` proto definition unchanged. `BomService` non-Excel methods (CRUD, substitute, etc.) unchanged

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| calamine `Cursor<Vec<u8>>` for XLSX not supported in current version | Verify calamine 0.32 API during U3 implementation; fall back to tempfile if needed |
| `BomExporter` formatting regression (cell styles lost during extraction) | Preserve all format helpers from `bom_service_impl.rs:82-170` verbatim; add BOM export integration test |
| Labor process `routing_results` info loss when flattening to `ImportResult` | Per design spec, routing_results is not needed by client; document the change |
| `HashMap` stale entries if import panics | Wrap import execution in `catch_unwind` or use `Drop` guard to clean active_imports map |
| Forwarding shims cause confusion during migration | Mark shims with `// TODO: remove after Phase 4` comments; delete in U9 |

---

## Documentation / Operational Notes

- Update `CLAUDE.md` "Adding a New Feature" section to note that Excel operations now use `implt/excel/` structs implementing `ExcelImportService`/`ExcelExportService` rather than being embedded in domain service traits
- No database migration required — this is a pure code refactor
- No configuration changes required
- No monitoring changes required

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-04-29-excel-service-unification-design.md](docs/superpowers/specs/2026-04-29-excel-service-unification-design.md)
- **Ideation document:** [docs/ideation/2026-04-29-excel-service-unification-ideation.md](docs/ideation/2026-04-29-excel-service-unification-ideation.md)
- Related code: `abt/src/lib.rs:70` (OnceLock), `abt-grpc/src/handlers/excel.rs` (string dispatch), `abt/src/service/product_excel_service.rs` (trait to replace)
- Related learnings: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`, `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`

## Deferred / Open Questions

### From 2026-04-29 review

- **工序导入路径校验缺失** — U6, U8 (P1, feasibility, confidence 75)

  迁移后 labor process handler 目前没有任何路径校验代码，file_path 可能绕过目录边界检查导致任意文件读取。计划未明确说明校验逻辑应放在 handler 还是 importer 中。

- **routing_results 和结构化行结果丢失** — U6, U8, System-Wide Impact (P1, feasibility, adversarial, confidence 100)

  统一 ImportResult 无法承载 proto ImportLaborProcessesResponse 中需要的 repeated ImportLaborProcessResult（含 row_number、process_name、operation、error_message）和 repeated ProductRoutingInfo routing_results。但计划同时声称 "API surface parity" 和 "Response types are unchanged"——与丢弃结构化数据矛盾。

- **active_imports HashMap 并发覆盖** — High-Level Technical Design — Handler Progress Storage (P1, adversarial, confidence 75)

  active_imports: Mutex<HashMap<String, Arc<ProgressTracker>>> 按 import_type 字符串做 key。如果两个用户同时导入同类型，第二次请求会覆盖第一个用户的 Arc<ProgressTracker>，导致第一个用户的进度查询看到第二个用户的数据。

- **注册表分发相比 match 语句没有简化** — U11 (P2, scope-guardian, adversarial, confidence 100)

  用 HashMap<ExportType, ExportFn> 注册表替换 match export_type，但新增导出类型仍需：(1) 创建结构体，(2) 添加枚举变体，(3) 在 new() 中注册闭包——和 match 语句的修改点数量相同。注册表引入了类型擦除复杂性、堆分配闭包开销，且忘记注册会变成运行时错误而非编译期错误。

- **BomExporter 需要返回 BOM 名称但 trait 只返回 Vec\<u8\>** — U7, High-Level Technical Design (P2, feasibility, adversarial, confidence 100)

  当前 BomService::export_to_bytes 返回 Result<(Vec<u8>, String)>，String 是 BOM 名称用于下载文件名。新 ExcelExportService trait 只返回 Result<Vec<u8>>。计划承认了此问题但未给出具体方案（"通过包装器或单独查询适配"）。
