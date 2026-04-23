# Labor Process Flat Model Restore

Revert the three-layer labor process model (labor_process + labor_process_group + bom_labor_cost) to a simple flat model (bom_labor_process) with product_code association, restoring simplicity and reducing maintenance errors.

## Background

The three-layer redesign (migration 021) introduced global process master, process groups, and BOM cost items with price snapshots. While architecturally richer, it proved overly complex in practice and error-prone during maintenance. The flat model where each product independently configures its own labor processes is simpler and better suited to actual usage patterns.

## Data Model

### `bom_labor_process` — BOM Labor Process (flat)

| Field | Type | Description |
|-------|------|-------------|
| id | BIGSERIAL PK | Auto-increment primary key |
| product_code | VARCHAR(100) NOT NULL | Product code, links to BOM's product |
| name | VARCHAR(255) NOT NULL | Process name |
| unit_price | DECIMAL(18,6) NOT NULL | Unit price |
| quantity | DECIMAL(18,6) NOT NULL DEFAULT 1 | Quantity |
| sort_order | INT NOT NULL DEFAULT 0 | Sort order |
| remark | TEXT | Remark |
| created_at | TIMESTAMPTZ NOT NULL DEFAULT NOW() | Created at |
| updated_at | TIMESTAMPTZ | Updated at |

**Constraints:** `UNIQUE(product_code, name)`

**Indexes:** `bom_labor_process(product_code)`

### Differences from original flat table

- Removed `site_id` and `language_id` columns (deprecated, removed in migration 009)
- `DECIMAL(18,6)` instead of `DECIMAL(12,2)` (matches project convention from migration 011)
- Only one index on `product_code` (removed `site_id, language_id` composite index)

## API Design

### gRPC Service: `AbtLaborProcessService`

| Method | Description |
|--------|-------------|
| `ListLaborProcesses` | List processes for a product_code with optional keyword filter and pagination |
| `CreateLaborProcess` | Create a process for a product (unique name per product_code) |
| `UpdateLaborProcess` | Update a process |
| `DeleteLaborProcess` | Delete a process by id and product_code |
| `ImportLaborProcesses` | Import processes from Excel (clear + bulk insert for the product) |
| `ExportLaborProcesses` | Export processes to Excel (stream) |

### Proto Messages

```protobuf
service AbtLaborProcessService {
  rpc ListLaborProcesses(ListLaborProcessesRequest) returns (LaborProcessListResponse);
  rpc CreateLaborProcess(CreateLaborProcessRequest) returns (U64Response);
  rpc UpdateLaborProcess(UpdateLaborProcessRequest) returns (BoolResponse);
  rpc DeleteLaborProcess(DeleteLaborProcessRequest) returns (U64Response);
  rpc ImportLaborProcesses(ImportLaborProcessesRequest) returns (ImportLaborProcessesResponse);
  rpc ExportLaborProcesses(ExportLaborProcessesRequest) returns (stream DownloadFileResponse);
}
```

| Message | Key Fields |
|---------|------------|
| `BomLaborProcessProto` | id, product_code, name, unit_price(string), quantity(string), sort_order, remark |
| `ListLaborProcessesRequest` | product_code(required), keyword(optional), page(optional), page_size(optional) |
| `CreateLaborProcessRequest` | product_code, name, unit_price, quantity, sort_order, remark |
| `UpdateLaborProcessRequest` | id, product_code, name, unit_price, quantity, sort_order, remark |
| `DeleteLaborProcessRequest` | id, product_code |
| `ImportLaborProcessesRequest` | file_path, product_code |
| `ImportLaborProcessesResponse` | success_count, failure_count, results(row-level) |
| `ExportLaborProcessesRequest` | product_code |

## Rust Models

```rust
struct BomLaborProcess {
    id: i64,
    product_code: String,
    name: String,
    unit_price: Decimal,
    quantity: Decimal,
    sort_order: i32,
    remark: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

struct CreateLaborProcessReq {
    product_code: String,
    name: String,
    unit_price: Decimal,
    quantity: Decimal,
    sort_order: i32,
    remark: Option<String>,
}

struct UpdateLaborProcessReq {
    id: i64,
    product_code: String,
    name: String,
    unit_price: Decimal,
    quantity: Decimal,
    sort_order: i32,
    remark: Option<String>,
}

struct ListLaborProcessQuery {
    product_code: String,
    keyword: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

struct LaborProcessImportResult {
    success_count: u64,
    failure_count: u64,
    results: Vec<ImportRowResult>,
}
```

## Repository Layer

```rust
// Reads
find_by_product_code(pool, product_code, keyword, page, page_size) -> (Vec<BomLaborProcess>, i64)
count_by_product_code(pool, product_code, keyword) -> i64

// Writes
insert(executor, req: &CreateLaborProcessReq) -> i64
update(executor, req: &UpdateLaborProcessReq) -> ()
delete(executor, id, product_code) -> u64

// Excel batch
delete_by_product_code(executor, product_code) -> u64
batch_insert(executor, product_code, items: &[(name, unit_price, quantity, sort_order, remark)]) -> ()
```

Query filter: `WHERE product_code = $1 AND (name ILIKE '%keyword%')` — keyword empty means no filter.

## Service Layer

```rust
#[async_trait]
trait LaborProcessService: Send + Sync {
    // Reads — uses internal PgPool
    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)>;

    // Writes — accepts Executor from handler transaction
    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64>;

    // Excel — import manages its own transaction internally
    async fn import_from_excel(&self, product_code: &str, file_path: &str) -> Result<LaborProcessImportResult>;
    async fn export_to_bytes(&self, product_code: &str) -> Result<Vec<u8>>;
}
```

## Excel Import/Export

### Import Flow

1. Parse Excel with calamine, headers: `工序名称 | 单价 | 数量 | 排序 | 备注`
2. Name normalization: trim, full-width to half-width, remove zero-width characters
3. Validation: name required, unit_price >= 0, quantity >= 0
4. Within transaction: `DELETE FROM bom_labor_process WHERE product_code = $1`, then `batch_insert`
5. Return row-level results

### Export Flow

1. Query all processes for the product_code, ordered by sort_order
2. Generate Excel with rust_xlsxwriter
3. Return as byte stream via ReceiverStream

### Excel Format

| 工序名称 | 单价 | 数量 | 排序 | 备注 |
|---------|------|------|------|------|
| 切割 | 15.50 | 1 | 1 | 激光切割 |

## Handler Layer

`LaborProcessHandler` implements tonic trait for `AbtLaborProcessService`.

| Method | Permission | Transaction |
|--------|-----------|-------------|
| list_labor_processes | LaborProcess + Read | None |
| create_labor_process | LaborProcess + Write | state.begin_transaction() |
| update_labor_process | LaborProcess + Write | state.begin_transaction() |
| delete_labor_process | LaborProcess + Write | state.begin_transaction() |
| import_labor_processes | LaborProcess + Write | Service-internal |
| export_labor_processes | LaborProcess + Read | None |

## File Changes

| File | Action |
|------|--------|
| `abt/migrations/023_revert_to_flat_labor_process.sql` | New — drop three-layer tables, create flat table |
| `proto/abt/v1/labor_process.proto` | Rewrite — flat model messages and RPCs |
| `abt/src/models/labor_process.rs` | Rewrite — BomLaborProcess and request structs |
| `abt/src/repositories/labor_process_repo.rs` | Rewrite — flat CRUD + batch operations |
| `abt/src/service/labor_process_service.rs` | Rewrite — simplified trait |
| `abt/src/implt/labor_process_service_impl.rs` | Rewrite — simplified impl |
| `abt-grpc/src/handlers/labor_process.rs` | Rewrite — simplified handler |

No changes needed: `lib.rs` (factory function signature compatible), `server.rs` (independent service already registered), `mod.rs` files (module names unchanged).

## Business Rules

- `UNIQUE(product_code, name)`: same process name cannot appear twice for the same product
- Import uses "clear + bulk insert" strategy: all existing processes for the product are replaced
- Validation: name required, unit_price >= 0, quantity >= 0
- Delete verifies `product_code` matches to prevent cross-product deletion
