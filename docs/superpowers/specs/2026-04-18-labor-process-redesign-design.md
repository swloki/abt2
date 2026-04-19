# Labor Process Redesign

Replace the existing `bom_labor_process` table and API with a three-layer model: process list, process group, and BOM labor cost.

## Background

Current labor cost management is coarse — each BOM must configure labor processes individually. When a labor cost changes, every BOM using that process must be updated manually. The new design introduces a global process list with centralized pricing and process groups for reuse across BOMs.

## Data Model

### `labor_process` — Process Master Table

| Field | Type | Description |
|-------|------|-------------|
| id | BIGSERIAL PK | Auto-increment primary key |
| name | VARCHAR NOT NULL UNIQUE | Process name, unique |
| unit_price | DECIMAL(12,6) NOT NULL | Unit price |
| remark | TEXT | Remark |
| created_at | TIMESTAMPTZ DEFAULT NOW() | Created at |
| updated_at | TIMESTAMPTZ | Updated at |

### `labor_process_group` — Process Group

| Field | Type | Description |
|-------|------|-------------|
| id | BIGSERIAL PK | Auto-increment primary key |
| name | VARCHAR NOT NULL UNIQUE | Group name, unique |
| remark | TEXT | Remark |
| created_at | TIMESTAMPTZ DEFAULT NOW() | Created at |
| updated_at | TIMESTAMPTZ | Updated at |

### `labor_process_group_member` — Process Group Member (join table)

| Field | Type | Description |
|-------|------|-------------|
| group_id | BIGINT NOT NULL REFERENCES labor_process_group(id) ON DELETE CASCADE | Group reference |
| process_id | BIGINT NOT NULL REFERENCES labor_process(id) ON DELETE RESTRICT | Process reference |
| sort_order | INT NOT NULL | Process order within group |
| PRIMARY KEY | (group_id, process_id) | Composite primary key |

### `bom_labor_cost` — BOM Labor Cost Items

| Field | Type | Description |
|-------|------|-------------|
| id | BIGSERIAL PK | Auto-increment primary key |
| bom_id | BIGINT NOT NULL | Associated BOM |
| process_id | BIGINT NOT NULL | Associated process |
| quantity | DECIMAL(12,6) NOT NULL DEFAULT 0 | Quantity |
| unit_price_snapshot | DECIMAL(12,6) | Price snapshot at time of set (frozen from labor_process.unit_price) |
| remark | TEXT | Remark (required when quantity is 0) |
| created_at | TIMESTAMPTZ DEFAULT NOW() | Created at |
| updated_at | TIMESTAMPTZ | Updated at |

### BOM Table Change

Add `process_group_id BIGINT` column to the `bom` table, pointing to the selected process group.

## API Design

### Process CRUD (`LaborProcessService`)

| Method | Description |
|--------|-------------|
| `ListLaborProcesses` | List all processes with pagination |
| `CreateLaborProcess` | Create process (unique name validation) |
| `UpdateLaborProcess` | Update process (name, price, remark). When `unit_price` changes, returns affected BOM count and bom_labor_cost item count in response. |
| `DeleteLaborProcess` | Delete process (reject if referenced by any process group) |

### Process Group CRUD (`LaborProcessGroupService`)

| Method | Description |
|--------|-------------|
| `ListLaborProcessGroups` | List all groups with basic info + member list (ordered by sort_order) |
| `CreateLaborProcessGroup` | Create group (unique name, validate all process_ids exist, set sort_order) |
| `UpdateLaborProcessGroup` | Update group (name, process member list with sort_order, remark) |
| `DeleteLaborProcessGroup` | Delete group (reject if referenced by any BOM) |

### BOM Labor Cost

| Method | Description |
|--------|-------------|
| `SetBomLaborCost` | Set BOM labor cost: accepts `bom_id`, `process_group_id`, and per-process `quantity`/`remark`. Clears old `bom_labor_cost` records then bulk inserts new ones. Freezes current `labor_process.unit_price` into `unit_price_snapshot` for each item. |
| `GetBomLaborCost` | Get BOM labor cost: returns process group info + each process's name, current unit price, snapshot price, quantity, subtotal (current price * quantity), snapshot subtotal (snapshot price * quantity), remark, and total cost. |

### Business Rules

- Deleting a process: reject if any `labor_process_group_member` references it (FK RESTRICT enforces this at DB level)
- Deleting a process group: reject if any BOM's `process_group_id` points to it (cascade deletes members via FK ON DELETE CASCADE)
- `SetBomLaborCost`: when quantity is 0 for a process, remark is required
- `SetBomLaborCost`: freezes current `labor_process.unit_price` into `bom_labor_cost.unit_price_snapshot` for audit trail
- Price changes in `labor_process` automatically propagate to all BOMs for current cost (price is fetched live), but snapshot preserves historical cost at time of last set

## Code Architecture

Following the project's layered pattern:

### New/Modified Files

| Layer | File | Change |
|-------|------|--------|
| Proto | `proto/abt/v1/labor_process.proto` | Replace existing — process, group, and BOM cost messages + service |
| Proto | `proto/abt/v1/bom.proto` | Add labor cost fields to BOM messages |
| Model | `abt/src/models/labor_process.rs` | `LaborProcess`, `LaborProcessGroup` structs |
| Model | `abt/src/models/bom.rs` | Add `process_group_id` and labor cost fields |
| Repository | `abt/src/repositories/labor_process_repo.rs` | Process CRUD + group CRUD + group member CRUD + BOM labor cost read/write |
| Service | `abt/src/service/labor_process_service.rs` | `LaborProcessService` trait covering all three areas |
| Impl | `abt/src/implt/labor_process_impl.rs` | Concrete implementation |
| Handler | `abt-grpc/src/handlers/labor_process.rs` | Replace existing handler |
| Migration | `abt/migrations/XXX_labor_process_redesign.sql` | Drop old `bom_labor_process`, create new tables, alter BOM table |

### Removed Code

- Old `bom_labor_process` related code in repository, service, impl, and handler layers

### Design Decisions

- Process and process group are managed in a single service to reduce file scatter given their tight coupling
- `labor_process_group_member` join table replaces JSONB array for process-group membership — provides FK referential integrity, explicit sort_order, and standard SQL queryability
- `bom_labor_cost` stores both quantities and price snapshots — current price is fetched live for real-time accuracy, while `unit_price_snapshot` preserves historical cost at time of last set for audit trail
- `UpdateLaborProcess` returns affected BOM count when price changes — gives operators situational awareness before confirming high-impact changes
