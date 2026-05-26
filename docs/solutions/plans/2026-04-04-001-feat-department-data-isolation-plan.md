---
title: "feat: Department-Based Data Isolation via Junction Table + JWT Pruning"
type: feat
status: active
date: 2026-04-04
deepened: 2026-04-04
origin: docs/brainstorms/2026-04-04-department-isolation-requirements.md
---

# feat: Department-Based Data Isolation

## Overview

Implement department-based data isolation using a `department_resource_access` junction table and login-time JWT permission pruning. Departments control which resource types users can see (table-level, not row-level). The pruning happens at JWT construction time, so `AuthContext::check_permission()` and all handlers remain unchanged.

## Problem Frame

Any user with `product:read` sees all products regardless of department. The system needs table-level visibility control: if department A has access to `product`, its users see ALL products. A junction table supports M:N (multiple departments can share a resource type). (See origin: `docs/brainstorms/2026-04-04-department-isolation-requirements.md`)

## Requirements Trace

- R1 — Junction table `department_resource_access(department_id, resource_code)` with FK on `department_id`
- R2 — Table-level visibility: department row for `product` = all users see ALL products
- R2b — Fail-closed: department with zero rows grants no business resource access
- R3 — 8 business resources subject to isolation: product, term, bom, warehouse, location, inventory, price, labor_process
- R4 — 5 system resources bypass filtering: user, role, permission, department, excel
- R5 — `is_default` column on departments table, exactly one default
- R5b — No default department or inactive default = fail-closed
- R6 — Users without departments fall back to default department at login time
- R7 — Default department configurable via same management API
- R8 — Both AuthRepo AND PermissionRepo `get_user_permission_codes()` apply department filtering with union semantics
- R9 — super_admin bypass unchanged (empty permissions list, short-circuit on `is_super_admin`)
- R10 — Filtering applies in login(), refresh_token(), get_user_claims(). Note: login() and refresh_token() issue JWTs (stale-JWT concern applies); get_user_claims() returns fresh Claims without JWT issuance, so department changes are reflected immediately on that path
- R11 — gRPC RPCs: SetDepartmentResources (full-overwrite), GetDepartmentResources
- R11b — Validate resource_codes against known 8 business types, reject unknowns, silently ignore system codes
- R11c — Validate department_id references existing active department
- R12 — SetDepartmentResources requires department:write, GetDepartmentResources requires department:read
- R13 — Default department deletable rejected by delete_department RPC
- R14 — Remove dead `department_id` field from Rust Resource model struct
- R15 — Migration seeds default department with all 8 business resource codes
- R16 — Migration seeds all existing departments with all 8 business resource codes
- R16b — Seeding atomic within single transaction

## Scope Boundaries

- Not doing: row-level isolation, cross-department sharing, hierarchical departments
- Not doing: audit logging for department resource access changes
- Not doing: modifying AuthContext::check_permission() logic
- Not doing: adding department_id columns to business entity tables
- Not doing: token revocation or force-logout
- Prerequisite: role permission proto refactor is complete — codebase compiles successfully with the new resource_code/action_code pattern (the plan document at `docs/superpowers/plans/2026-04-01-role-permission-refactor.md` is stale but the work is done)
- Pre-existing issue: `abt/src/tests/migration_test.rs` contains tests referencing `resources`, `actions`, `permissions` tables dropped in migration 015. These tests are stale and will fail. Fix or remove them before starting implementation.

## Context & Research

### Relevant Code and Patterns

- **Migration pattern**: Sequential numbered SQL files in `abt/migrations/` with `-- +migrate Up`/`-- +migrate Down` blocks, `ON CONFLICT DO NOTHING` for seeding
- **Repository pattern**: Static struct with `pub async fn method(pool: &PgPool, ...) -> Result<T>` and `Executor<'_>` for transactional operations. See `abt/src/repositories/department_repo.rs` for `get_user_department_ids()` and `assign_departments()` with `unnest()` batch insert
- **Service pattern**: `#[async_trait]` trait in `abt/src/service/`, impl in `abt/src/implt/` with `Arc<PgPool>`. All methods accept `operator_id: Option<i64>`. See `abt/src/service/department_service.rs` and `abt/src/implt/department_service_impl.rs`
- **Handler pattern**: `extract_auth(&request)?` → `auth.check_permission(resource, action)` → `AppState::get().await` → service call. Transaction: `state.begin_transaction()` → service call → `tx.commit()`. See `abt-grpc/src/handlers/department.rs`
- **Proto pattern**: Package `abt.v1`, import `base.proto`, CRUD RPCs with request/response messages. See `proto/abt/v1/department.proto`
- **Server registration**: `DepartmentServiceServer::with_interceptor(handler, auth_interceptor)` in `abt-grpc/src/server.rs`
- **Factory pattern**: `pub fn get_xxx_service(ctx: &AppContext) -> impl XxxService` in `abt/src/lib.rs`
- **Auth flow**: JWT Claims → auth interceptor → `AuthContext` inserted into request extensions. `AuthRepo::get_user_permission_codes()` joins `user_roles` → `role_permissions` and returns `Vec<String>` of `"resource:action"` codes
- **PermissionRepo**: Duplicate `get_user_permission_codes()` in `abt/src/repositories/permission_repo.rs` used by `PermissionServiceImpl` for gRPC permission API — also needs filtering per R8. Additionally, `PermissionRepo::check_permission()` does direct SQL against `role_permissions` without department filtering — this must also be updated or explicitly documented as a non-enforcement path (admin API only)
- **Dead code in permission.rs**: The `resources`, `actions`, `permissions` tables were dropped in migration 015. The `Resource`, `Permission`, and related model structs may be partially or fully dead. Audit before cleanup
- **Resource model dead code**: `abt/src/models/permission.rs` Resource struct has `department_id: Option<i64>` referencing the dropped `resources` table

### Institutional Learnings

- The `resources` table was dropped in migration 015. Any reference to `resources.department_id` is stale
- The junction table pattern is structurally identical to existing `role_permissions(resource_code, action_code)` — follow that convention
- Excel import creates business data visible to ALL departments with that resource type (accepted table-level tradeoff)

## Key Technical Decisions

- **Junction table over column**: `department_resource_access(department_id, resource_code)` supports M:N, avoids modifying business tables
- **JWT pruning over runtime filtering**: Zero downstream changes. Tradeoff: re-login required for changes to take effect (max stale window = `jwt_expiration_hours`)
- **Both AuthRepo and PermissionRepo filtered**: Prevents inconsistency between JWT permissions and permission API responses
- **Application-level default enforcement**: Service layer checks/creates default department. Simpler than partial unique index, more portable
- **Add RPCs to existing department.proto**: Logically department management, avoids proto proliferation
- **New DepartmentResourceAccessRepo**: Separate from DepartmentRepo for single responsibility. Both AuthRepo and PermissionRepo call it for resource code resolution

## Open Questions

### Deferred to Implementation

- Exact method signatures for the new repo/service — follow existing patterns
- Whether to add department filtering logic as a standalone function called by both repos or inline the SQL — implementation-time choice based on code clarity
- Exact `unnest()` vs batch-insert pattern for SetDepartmentResources — follow `assign_departments` pattern

## Implementation Units

- [ ] **Unit 1: Database migration**

**Goal:** Create the `department_resource_access` junction table, add `is_default` to departments, seed default department and all existing departments with full resource access.

**Requirements:** R1, R5, R15, R16, R16b

**Dependencies:** None (prerequisite for all other units)

**Files:**
- Create: `abt/migrations/016_department_resource_access.sql`
- Test: `abt/tests/department_resource_access_migration.rs` (if integration test pattern exists)

**Approach:**
- Single migration file with `-- +migrate Up` / `-- +migrate Down` blocks
- Create `department_resource_access` table with composite PK `(department_id, resource_code)`, FK on `department_id` referencing `departments(department_id) ON DELETE CASCADE`, index on `department_id`
- Add `is_default BOOLEAN NOT NULL DEFAULT false` to `departments` table
- Seed: find or create a default department (e.g., code `'default'`), set `is_default = true`
- Seed: insert all 8 business resource codes for every existing department and the default department
- All seeding in a single transaction block for atomicity (R16b)
- Down migration: drop the junction table, drop `is_default` column

**Patterns to follow:**
- `abt/migrations/012_add_department_tables.sql` — table creation, junction table, seeding
- `abt/migrations/015_auth_system.sql` — batch seed data with `ON CONFLICT DO NOTHING`

**Test scenarios:**
- Happy path: migration runs, `department_resource_access` table exists with rows for all existing departments × 8 resource codes
- Edge case: fresh database with zero departments — default department created with all 8 resources
- Edge case: migration is idempotent — running twice produces same result (ON CONFLICT)
- Integration: down migration cleanly reverses all changes

**Verification:**
- Migration applies cleanly against a database with existing departments from migration 012 seed data
- `SELECT COUNT(*) FROM department_resource_access` = number of departments × 8
- Exactly one department has `is_default = true`

---

- [ ] **Unit 2: Model and repository layer**

**Goal:** Create the Rust model for department resource access and the repository with CRUD operations and query methods.

**Requirements:** R1, R2, R2b, R5, R5b

**Dependencies:** Unit 1

**Files:**
- Create: `abt/src/models/department_resource_access.rs`
- Modify: `abt/src/models/department.rs` — add `is_default: bool` field to `Department` struct and `FromRow` impl
- Create: `abt/src/repositories/department_resource_access_repo.rs`
- Modify: `abt/src/models/mod.rs` — add new model module
- Modify: `abt/src/repositories/mod.rs` — add new repo module
- Test: `abt/src/repositories/department_resource_access_repo.rs` (inline tests or separate test file following project convention)

**Approach:**
- Model: `DepartmentResourceAccess` struct with `department_id: i64`, `resource_code: String`, plus `FromRow` derive
- Repo methods:
  - `get_department_resources(pool, department_id) -> Result<Vec<String>>` — returns resource_codes for a department
  - `get_departments_accessible_resources(pool, department_ids: &[i64]) -> Result<Vec<String>>` — returns UNION of resource_codes across all given department IDs (union semantics for multi-department users, R8)
  - `get_default_department_id(pool) -> Result<Option<i64>>` — finds department where `is_default = true`
  - `set_department_resources(executor, department_id, resource_codes: &[String]) -> Result<()>` — delete+insert in transaction (full-overwrite semantics, R11)
  - `get_all_business_resource_codes() -> Vec<&'static str>` — returns the hardcoded list of 8 business resource codes from R3. Reference the existing `RESOURCES` list in `abt/src/models/resources.rs` as the single source of truth for resource codes and their business/system classification
- Follow `DepartmentRepo` pattern: static struct, `pool: &PgPool` for reads, `Executor<'_>` for writes
- Use `unnest()` for batch insert like `assign_departments` in `DepartmentRepo`

**Patterns to follow:**
- `abt/src/repositories/department_repo.rs` — static repo struct, `get_user_department_ids()`, `assign_departments()` with `unnest()`
- `abt/src/models/permission.rs` — model struct with `FromRow`

**Test scenarios:**
- Happy path: set resources for a department, then get them back — returns same list
- Happy path: get accessible resources for multiple departments — returns union (no duplicates)
- Edge case: department with zero resources — `get_department_resources` returns empty vec (fail-closed, R2b)
- Edge case: no default department — `get_default_department_id` returns `None` (R5b)
- Edge case: `set_department_resources` with empty array — deletes all rows (full-overwrite)
- Error path: set resources for nonexistent department_id — FK constraint error from DB

**Verification:**
- All repo methods compile against the migration schema
- Union query returns correct results for multi-department users

---

- [ ] **Unit 3: Service trait and implementation**

**Goal:** Define the service trait for department resource access management and implement it.

**Requirements:** R11, R11b, R11c, R12, R13

**Dependencies:** Unit 2

**Files:**
- Create: `abt/src/service/department_resource_access_service.rs`
- Create: `abt/src/implt/department_resource_access_service_impl.rs`
- Modify: `abt/src/service/mod.rs` — add new service module
- Modify: `abt/src/implt/mod.rs` — add new impl module
- Modify: `abt/src/lib.rs` — add factory function `get_department_resource_access_service`
- Test: inline tests or `abt/tests/department_resource_access_service.rs`

**Approach:**
- Service trait methods:
  - `set_department_resources(operator_id: Option<i64>, department_id: i64, resource_codes: Vec<String>, executor: Executor<'_>) -> Result<()>`
  - `get_department_resources(department_id: i64) -> Result<Vec<String>>`
- `set_department_resources` validation (R11b, R11c):
  - Validate each resource_code is in the known 8 business types list — reject entire request on unknown
  - Filter out system resource codes silently (don't store them)
  - Validate department_id references an existing, active department — return error if not
  - If target department is the default department (`is_default = true`), allow modification (R13) but ensure seeding stays correct
- Follow existing service pattern: `operator_id` for audit, `Executor<'_>` for transaction support, `#[async_trait]`
- Factory function in `lib.rs` following existing pattern: `get_department_resource_access_service(ctx: &AppContext)`

**Patterns to follow:**
- `abt/src/service/department_service.rs` — service trait with async_trait
- `abt/src/implt/department_service_impl.rs` — impl with Arc<PgPool>, operator_id, executor
- `abt/src/lib.rs` — factory functions

**Test scenarios:**
- Happy path: set resources for a valid department, get them back
- Happy path: set resources for the default department — allowed
- Error path: set resources with an unknown resource code — rejected with error
- Edge case: set resources with system resource codes mixed in — system codes silently filtered, only business codes stored
- Error path: set resources for nonexistent department_id — error returned
- Error path: set resources for inactive department — error returned

**Verification:**
- Service compiles, factory function accessible from `lib.rs`
- Validation logic correctly filters and rejects

---

- [ ] **Unit 4: Proto definition and handler**

**Goal:** Define gRPC messages and RPCs, create handler, register in server.

**Requirements:** R11, R12, R13

**Dependencies:** Unit 3

**Files:**
- Modify: `proto/abt/v1/department.proto` — add messages and RPCs
- Modify: `abt-grpc/src/handlers/department.rs` — add handler methods for new RPCs
- Modify: `abt/src/implt/department_service_impl.rs` — add is_default check to delete()
- Modify: `abt-grpc/src/server.rs` — handler already registered (same DepartmentService)
- Test: manual gRPC reflection or integration test

**Approach:**
- Add to existing `department.proto` (R11 extends department management):
  - `SetDepartmentResourcesRequest { int64 department_id = 1; repeated string resource_codes = 2; }`
  - `SetDepartmentResourcesResponse { repeated string resource_codes = 1; }`
  - `GetDepartmentResourcesRequest { int64 department_id = 1; }`
  - `GetDepartmentResourcesResponse { repeated string resource_codes = 1; }`
  - Add RPCs to existing `DepartmentService`: `SetDepartmentResources`, `GetDepartmentResources`
- Handler methods follow existing pattern:
  - `set_department_resources`: extract_auth → check_permission("department", "write") → validate → begin_tx → service call → commit
  - `get_department_resources`: extract_auth → check_permission("department", "read") → service call
  - Return stored resource_codes in response
- Modify `DepartmentServiceImpl::delete()` to check `is_default` and reject deletion of the default department (R13). Put this check in the service layer (not handler) — follows existing pattern where business validation lives in services

**Patterns to follow:**
- `proto/abt/v1/department.proto` — message and RPC definitions
- `abt-grpc/src/handlers/department.rs` — extract_auth, check_permission, transaction pattern
- `abt-grpc/src/server.rs` — service registration (no change needed since same service)

**Test scenarios:**
- Happy path: call SetDepartmentResources with valid codes → stored, response matches
- Happy path: call GetDepartmentResources → returns stored codes
- Error path: call SetDepartmentResources without department:write → permission denied
- Error path: call GetDepartmentResources without department:read → permission denied
- Integration: set resources, then get them — round-trip consistent

**Verification:**
- Proto compiles, handler methods registered, gRPC reflection shows new RPCs
- Set/Get round-trip works end-to-end

---

- [ ] **Unit 5: Permission pruning in AuthRepo and PermissionRepo**

**Goal:** Apply department-based resource filtering to both permission resolution paths.

**Requirements:** R2b, R4, R5b, R6, R8, R9, R10

**Dependencies:** Unit 2

**Files:**
- Modify: `abt/src/repositories/auth_repo.rs` — modify `get_user_permission_codes()`
- Modify: `abt/src/repositories/permission_repo.rs` — modify `get_user_permission_codes()` AND `check_permission()`
- Test: inline tests or integration tests

**Approach:**
- Core logic (shared between both repos):
  1. If user is super_admin: return all role permissions (unchanged, R9)
  2. Get user's department IDs via `DepartmentRepo::get_user_department_ids()`
  3. If no departments: resolve default department via `DepartmentResourceAccessRepo::get_default_department_id()`
  4. If still no department (R5b): filter out all business resource permissions (fail-closed)
  5. Get accessible resource codes via `DepartmentResourceAccessRepo::get_departments_accessible_resources()` (union)
  6. Filter role permissions: keep if resource_code is system resource (R4) OR resource_code is in accessible set
- **Critical: `PermissionRepo::check_permission()` must also apply department filtering.** This method queries `role_permissions` directly by resource_code + action_code, bypassing the JWT-filtered permissions. Without filtering, the gRPC permission API would report a user has access to resources their department cannot see. Add department resource code check to the `check_permission` query or add a pre-check using `get_departments_accessible_resources`
- Implementation options (deferred to implementer):
  - Option A: Add the filtering SQL directly to the existing query (JOIN with department_resource_access)
  - Option B: Query role permissions first, then filter in Rust using the accessible resource set
  - Option B is simpler and more testable — recommend this
- `PermissionRepo::get_user_permission_codes()` needs the same logic but currently only takes `(pool, user_id)` — the implementation can call `DepartmentResourceAccessRepo` methods directly since repos are static

**Patterns to follow:**
- `abt/src/repositories/auth_repo.rs` — existing `get_user_permission_codes()` query
- `abt/src/repositories/permission_repo.rs` — duplicate query that also needs filtering
- `abt/src/repositories/department_repo.rs` — `get_user_department_ids()` for department resolution

**Test scenarios:**
- Happy path: user in department with product+inventory access, role grants product:read+warehouse:read → returned list contains only product:read (warehouse not in department's accessible set)
- Happy path: user in two departments (Electronics: product, Production: product+bom) → returns product+bom permissions (union)
- Happy path: super_admin → all role permissions returned unchanged
- Happy path: refresh_token returns department-filtered permissions (R10)
- Happy path: get_user_claims returns department-filtered permissions (R10)
- Edge case: user with no departments, default department exists with product access → gets product permissions only
- Edge case: user with no departments, no default department → no business resource permissions (fail-closed, R5b)
- Edge case: department with zero configured resources → no business permissions (fail-closed, R2b)
- Integration: system resource permissions (user:read, role:write) always pass through regardless of department config
- Integration: PermissionRepo::check_permission() respects department filtering — returns false for business resources outside user's department scope
- Error path: department resolution query fails → fail-closed (no business resource permissions)

**Verification:**
- Both `AuthRepo::get_user_permission_codes()` and `PermissionRepo::get_user_permission_codes()` return department-filtered results
- JWT tokens issued after login contain only department-accessible permissions
- Permission API endpoints report consistent filtered results

---

- [ ] **Unit 6: Cleanup dead code**

**Goal:** Remove stale `department_id` field from the Resource model struct and audit broader dead code from dropped tables.

**Requirements:** R14

**Dependencies:** None (independent, but best done last to avoid merge conflicts)

**Files:**
- Modify: `abt/src/models/permission.rs` — remove `department_id` field, audit and remove dead structs
- Modify: `abt-grpc/src/handlers/convert.rs` — remove dead conversion impls
- Modify: `abt/src/tests/migration_test.rs` — remove or fix tests referencing dropped tables

**Approach:**
- Remove `pub department_id: Option<i64>` from `Resource` struct and `FromRow` impl
- Audit all structs in `permission.rs` that reference the dropped `resources`/`permissions`/`actions` tables: `Resource`, `Permission`, `PermissionInfo`, `ResourceGroup`, `PermissionGroup`
- Check if any handler or convert.rs code still uses these structs — if so, keep the struct but remove dead fields; if not, remove entirely
- Remove or update stale migration tests that assert existence of dropped tables

**Patterns to follow:**
- Standard Rust struct field removal

**Test scenarios:**
- Test expectation: none — pure dead code removal, no behavioral change. Code compiles after removal.

**Verification:**
- `cargo build` succeeds with no references to the removed field
- `cargo test` passes with stale migration tests fixed or removed
- Grep for `department_id` across the codebase returns zero hits

## System-Wide Impact

- **Interaction graph:** JWT login/refresh path, permission API endpoints, department management RPCs. No handler-level changes.
- **Error propagation:** Department filtering failures in the login path should fail-closed (deny access) rather than fail-open. If the department resolution query fails, the user should not get unfiltered permissions.
- **State lifecycle risks:** JWT carries stale department state until re-login. Migration seeding must be atomic to prevent partial states where some departments have access and others don't.
- **API surface parity:** The new RPCs extend the existing `DepartmentService` proto. No other interfaces need the same change.
- **Integration coverage:** End-to-end test should verify: login → JWT contains only department-filtered permissions → handler check_permission succeeds/fails accordingly.
- **Unchanged invariants:** `AuthContext::check_permission()` logic is untouched. All existing handler permission checks work identically — they just receive a shorter permissions list from the JWT.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Role permission proto refactor blocks compilation | Complete that refactor first or verify it compiles before starting |
| Migration fails partway, leaving partial seeding | All seeding in single transaction (R16b) |
| Default department accidentally deleted | Handler rejects deletion of `is_default=true` department (R13) |
| Stale JWT retains old department access after admin changes | Accepted tradeoff — bounded by `jwt_expiration_hours`. Token revocation can be added later |
| Excel import creates data visible to all departments | Accepted table-level tradeoff — documented in dependencies |
| `PermissionRepo::check_permission()` bypasses department filtering if not updated | Plan includes it in Unit 5 scope — must apply same filtering as `get_user_permission_codes()` |
| Stale migration tests referencing dropped tables | Fix or remove as pre-implementation cleanup |

## Documentation / Operational Notes

- After migration, verify all existing departments have 8 rows in `department_resource_access`
- After deployment, confirm JWT tokens for non-super-admin users contain only department-accessible permissions
- The `jwt_expiration_hours` value determines the maximum stale-access window — document this for operators

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-04-department-isolation-requirements.md](docs/brainstorms/2026-04-04-department-isolation-requirements.md)
- Related code: `abt/src/repositories/auth_repo.rs`, `abt/src/repositories/permission_repo.rs`, `abt/src/repositories/department_repo.rs`
- Related plans: `docs/superpowers/plans/2026-04-01-role-permission-refactor.md` (compilation prerequisite)
- Ideation: `docs/ideation/2026-04-04-department-resource-access-ideation.md`
