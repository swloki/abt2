# BOM Service Cleanup Design

Date: 2026-05-03

Three refactorings targeting `bom_service_impl.rs` and its callers: N+1 query elimination, redundant state removal, and API signature cleanup.

---

## 1. N+1 Query Optimization in `substitute_product`

**Problem**: When `bom_id=None`, the method fires N individual `BomRepo::find_by_id` calls for permission checks, then N `find_by_bom_id_for_update` calls, then M per-node UPDATEs — total 3N+M queries.

**Solution**: Batch queries at the SQL level.

### New repo methods

**`BomRepo::find_accessible_boms_by_product`**:
```sql
SELECT {bom columns} FROM bom b
JOIN bom_nodes bn ON bn.bom_id = b.bom_id
WHERE bn.product_id = $1
  AND (b.status = 'published' OR b.created_by = $2)
FOR UPDATE
```
One query replaces N `find_by_id` + permission check loops.

**`BomNodeRepo::find_by_bom_ids_and_product`**:
```sql
SELECT {node columns} FROM bom_nodes
WHERE bom_id = ANY($1) AND product_id = $2
FOR UPDATE
```
One query replaces N `find_by_bom_id_for_update` loops, only returning matching nodes.

### Service rewrite

- `bom_id=Some` path: unchanged (single-BOM operation)
- `bom_id=None` path: 1 batch BOM query + 1 batch node query + M node UPDATEs

**Query count**: 3N+M → 2+M

### Files changed

- `abt/src/repositories/bom_repo.rs` — add `find_accessible_boms_by_product`
- `abt/src/repositories/bom_node_repo.rs` — add `find_by_bom_ids_and_product`
- `abt/src/implt/bom_service_impl.rs` — rewrite `substitute_product`

---

## 2. Remove `BomDetail.created_by` Redundancy

**Problem**: `BomDetail.created_by` duplicates `Bom.created_by`. After the JSONB-to-relational migration, `BomDetail` is no longer deserialized from a column — it's constructed fresh with `created_by` copied from the parent `Bom`.

**Solution**: Remove the redundant field end-to-end.

### Changes

1. **`BomDetail` struct** — remove `created_by` field and the `deserialize_created_by` function
2. **`build_bom_detail`** — remove `created_by` parameter, return `BomDetail { nodes }`
3. **`bom.proto`** — remove `created_by` from `BomDetailProto` message
4. **`convert.rs`** — `BomResponse.created_by` sourced from `bom.created_by` directly (not from detail)
5. All callers of `build_bom_detail` (`find`, `publish`, `unpublish`) — drop the `created_by` argument

### Breaking change

Proto field removal is breaking for any client reading `bom_detail.created_by`. Frontend must use `bom.created_by` instead.

### Files changed

- `abt/src/models/bom.rs`
- `abt/src/implt/bom_service_impl.rs`
- `proto/abt/v1/bom.proto`
- `abt-grpc/src/generated/abt.v1.rs` (auto-regenerated)
- `abt-grpc/src/handlers/convert.rs`

---

## 3. Remove Unused `executor` Parameters

**Problem**: `get_leaf_nodes` and `get_product_code` accept `executor: Executor<'_>` in their trait signature but the implementation ignores it (uses `self.pool` directly). This misleads readers into thinking the operation participates in the caller's transaction.

**Solution**: Remove `executor` from both trait and impl signatures. Callers stop passing it.

### Changes

1. **`BomService` trait** — remove `executor` from `get_leaf_nodes` and `get_product_code`
2. **`BomServiceImpl`** — remove `_executor` parameter
3. **Handler** — stop passing executor when calling these methods; no transaction needed for these read-only operations

### Consistency

After this change, all read-only methods (`query`, `exists_name`, `get_leaf_nodes`, `get_product_code`, `get_bom_cost_report`) use `self.pool` without executor. All write methods (`create`, `update`, `delete`, `add_node`, `update_node`, `delete_node`, `swap_node_position`, `save_as`, `substitute_product`, `publish`, `unpublish`) accept executor.

### Files changed

- `abt/src/service/bom_service.rs`
- `abt/src/implt/bom_service_impl.rs`
- `abt-grpc/src/handlers/bom.rs`
