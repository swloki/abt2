---
title: "refactor: Simplify Permission System — Remove Department from Permission Checks"
type: refactor
status: active
date: 2026-04-17
origin: docs/brainstorms/2026-04-17-simplify-permission-requirements.md
---

# Refactor: Simplify Permission System — Remove Department from Permission Checks

## Overview

Remove department as a dimension from the permission check system. Users have global roles; roles define what users can do across the entire system. Departments remain as organizational concepts only. This involves simplifying JWT Claims, AuthContext, permission check logic, the `require_permission` macro, removing scoped-role endpoints, and a database migration.

## Problem Frame

The current Scoped Roles system requires department context for every permission check, creating friction for multi-department users. In practice, `department_resource_access` gives all departments all resources, making the department layer ineffective. The business requirement is simpler: permissions follow the user, not the department. (See origin: `docs/brainstorms/2026-04-17-simplify-permission-requirements.md`)

## Requirements Trace

- R1. JWT Claims: remove `current_department_id` and `dept_roles`, add `role_ids: Vec<i64>`
- R2. AuthContext: same simplification as R1
- R3. Remove department selection at login (remove `resolve_default_department`)
- R4. Business permission check: super_admin bypass → RolePermissionCache lookup
- R5. Remove `belongs_to_department` and `DeptResourceAccessCache` from checks
- R6. `require_permission` macro: external signature unchanged, remove `department_id` parameter internally
- R7. System resource checks unchanged
- R8. Reuse existing `user_roles` table (migration 010)
- R9. Remove `user_department_roles` table
- R10. Remove `department_resource_access` table
- R11. Keep `departments` and `user_departments` tables
- R12. Remove `switch_department` gRPC endpoint
- R13. Role assignment via existing UserRepo (super_admin only)
- R14. Keep basic department CRUD APIs
- R14a. Remove 5 scoped-role endpoints from department handler
- R15. Migrate `user_department_roles` → `user_roles` (role union, clear stale data first)
- R16. Force all users to re-login on deploy (no JWT backward compatibility)
- R17. Remove `DeptResourceAccessCache` and global singleton
- R18. Keep `RolePermissionCache`
- R19. Audit all `user_roles` query paths for consistency

## Scope Boundaries

- No changes to `roles` table structure or `role_permissions` table
- No changes to business data tables (no `department_id` added)
- No frontend permission map push
- No new permission models (ABAC, tags, etc.)
- Department membership APIs remain but are decoupled from permissions

## Context & Research

### Relevant Code and Patterns

- **Claims/AuthContext**: `abt/src/models/auth.rs` — `dept_roles: HashMap<String, Vec<i64>>` + `current_department_id: Option<i64>` to be replaced with `role_ids: Vec<i64>`
- **Permission check**: `abt-grpc/src/permissions/mod.rs` — `check_permission_for_resource` dispatches to system/business paths; `check_business_permission` does 3-step dept check
- **Macro**: `abt-macros/src/lib.rs` — generates `check_permission_for_resource(&auth, resource.code(), action.code(), auth.current_department_id)`. Must preserve `Box::pin` detection logic
- **Auth service**: `abt/src/implt/auth_service_impl.rs` — `build_claims`, `login`, `switch_department` (lines 240-276)
- **Cache**: `abt/src/permission_cache.rs` — `DeptResourceAccessCache` (lines 153-206) to remove; `RolePermissionCache` to keep
- **Repos**: `abt/src/repositories/department_resource_access_repo.rs` and `user_department_role_repo.rs` to delete
- **Proto**: `proto/abt/v1/auth.proto` line 15 (SwitchDepartment), `proto/abt/v1/department.proto` lines 22-28, 84-131 (5 RPCs + 9 messages)
- **Handlers**: `abt-grpc/src/handlers/department.rs` lines 263-396 (5 scoped-role handlers + helpers)

### Institutional Learnings

- The `require_permission` macro uses `Box::pin(async move { ... })` detection to penetrate `#[tonic::async_trait]` transformations — this dual-path logic must be preserved
- `PermissionCode` trait in `abt-grpc/src/permissions/mod.rs` bridges proto enums to runtime strings — must be preserved
- `user_roles` table exists from migration 010; migration 017 preserved it ("Do NOT drop user_roles yet") — it contains stale data that must be cleared before migration

### Key Constraints

- `abt` crate cannot reference `abt-grpc` types (dependency boundary)
- All 104 `require_permission` call sites must compile after macro change
- Claims struct is serde-deserialized — removing fields without backward compat means old JWTs fail

## Key Technical Decisions

- **Delete `department_id` parameter from macro call** (not keep as `Option`): Cleaner API, no dead parameter, compile-time guarantees catch all call sites
- **Force re-login** (no JWT compat): Simplest deployment, avoids dual-format Claims complexity
- **Role union migration**: Accept that multi-department users may gain broader permissions — verified acceptable by product decision
- **Keep `check_permission_for_resource` as dispatch function**: It handles system vs business resource routing; just remove `department_id` parameter and simplify business path

## Open Questions

### Resolved During Planning

- Macro `department_id` handling: delete parameter entirely, `check_permission_for_resource` signature changes from 4 args to 3
- Proto cleanup: remove 5 RPCs + SwitchDepartment + all associated messages
- PermissionRepo paths: these already query `user_roles` directly, will continue to work — just ensure consistency

### Deferred to Implementation

- Exact `role_ids` population in `build_claims`: query `user_roles` table via existing `AuthRepo` or `UserRepo` method
- Whether to keep `PermissionRepo::check_permission` and `PermissionRepo::get_user_permission_codes` methods (they query `user_roles` directly) — decide during implementation based on usage audit
- Frontend coordination: confirm which frontend APIs consume `switch_department` and scoped-role endpoints

## Implementation Units

- [ ] **Unit 1: Core Data Structures** (R1, R2)

**Goal:** Update Claims and AuthContext to use global `role_ids` instead of `dept_roles`/`current_department_id`

**Requirements:** R1, R2

**Dependencies:** None

**Files:**
- Modify: `abt/src/models/auth.rs`
- Modify: `abt/src/tests/auth_tests.rs`

**Approach:**
- In `Claims`: remove `dept_roles` and `current_department_id` fields, add `role_ids: Vec<i64>`
- In `AuthContext`: same changes; remove `belongs_to_department()` and `get_dept_role_ids()` methods; add `has_role(role_id: i64) -> bool` if useful
- Update test fixtures and test cases: remove dept_roles/current_department_id tests, add role_ids tests

**Patterns to follow:**
- Existing `is_super_admin()` method pattern for new role helper methods

**Test scenarios:**
- Happy path: AuthContext with multiple role_ids correctly reports roles
- Edge case: AuthContext with empty role_ids
- Error path: is_super_admin() still works with new struct shape

**Verification:**
- `cargo test -p abt -- auth_tests` passes
- `Claims` and `AuthContext` compile with new fields

---

- [ ] **Unit 2: Permission Check Simplification** (R4, R5, R6)

**Goal:** Remove department logic from permission checks and update the macro

**Requirements:** R4, R5, R6, R7

**Dependencies:** Unit 1

**Files:**
- Modify: `abt-grpc/src/permissions/mod.rs`
- Modify: `abt-macros/src/lib.rs`

**Approach:**
- `check_permission_for_resource`: remove `department_id: Option<i64>` parameter (3 args: auth, resource, action)
- `check_business_permission`: remove department_id parameter, simplify to: (1) super_admin → allow, (2) get `auth.role_ids`, (3) `RolePermissionCache.has_permission(&role_ids, resource, action)`
- Remove `check_business_permission`'s dept membership check, dept resource visibility check
- Keep `check_system_permission` unchanged
- Macro: change generated call from `check_permission_for_resource(&auth, #resource.code(), #action.code(), auth.current_department_id)` to `check_permission_for_resource(&auth, #resource.code(), #action.code())`
- Preserve `Box::pin` detection logic and `extract_request_ident` unchanged

**Patterns to follow:**
- Existing `RolePermissionCache.has_permission(role_ids, resource, action)` — already supports multi-role union

**Test scenarios:**
- Happy path: regular user with manager role passes business resource write check
- Happy path: regular user with staff role passes read check, fails write check
- Happy path: super_admin bypasses all business checks
- Edge case: user with no role_ids fails all business checks
- Integration: all 104 `require_permission` annotated handlers compile

**Verification:**
- `cargo build` succeeds across all crates
- `cargo test -p abt-grpc -- permissions::tests` passes

---

- [ ] **Unit 3: Auth Service Refactoring** (R3, R12, R16)

**Goal:** Rewrite login/claims flow to use global roles, remove switch_department

**Requirements:** R3, R12, R16

**Dependencies:** Unit 1

**Files:**
- Modify: `abt/src/implt/auth_service_impl.rs`
- Modify: `abt/src/service/auth_service.rs`
- Modify: `abt-grpc/src/interceptors/auth.rs`
- Modify: `abt/src/repositories/auth_repo.rs`

**Approach:**
- `build_claims`: remove `current_department_id` parameter, add role_ids from user_roles query
- `login`: remove `resolve_default_department` call; query user's roles from `user_roles` table instead of `user_department_roles`; build claims with `role_ids`
- `refresh_token`: same simplification — no dept context to preserve
- `get_user_claims`: same pattern
- Remove `resolve_default_department` method entirely
- Remove `switch_department` method from impl and trait
- Auth interceptor: construct AuthContext with `role_ids` from claims instead of `dept_roles`/`current_department_id`
- `AuthRepo`: modify `get_user_dept_roles` to return `Vec<i64>` (role IDs) from `user_roles` table, or add a new `get_user_role_ids` method

**Patterns to follow:**
- Existing `AuthRepo::get_user_dept_roles` query pattern for the new `get_user_role_ids`

**Test scenarios:**
- Happy path: login returns JWT with `role_ids` field
- Happy path: refresh_token preserves role_ids without department context
- Edge case: user with no roles gets empty `role_ids` in JWT
- Error path: user not found during login still returns proper error

**Verification:**
- `cargo build -p abt -p abt-grpc` succeeds
- Login flow produces valid JWT with `role_ids`

---

- [ ] **Unit 4: Proto & Handler Cleanup** (R12, R14, R14a)

**Goal:** Remove scoped-role proto definitions and handler implementations

**Requirements:** R12, R14, R14a

**Dependencies:** Unit 3

**Files:**
- Modify: `proto/abt/v1/auth.proto`
- Modify: `proto/abt/v1/department.proto`
- Modify: `abt-grpc/src/handlers/auth.rs`
- Modify: `abt-grpc/src/handlers/department.rs`

**Approach:**
- `auth.proto`: remove `SwitchDepartment` RPC and `SwitchDepartmentRequest` message
- `department.proto`: remove 5 RPCs (`AssignUserDepartmentRoles`, `RemoveUserDepartmentRoles`, `GetUserDepartmentRoles`, `SetDepartmentResources`, `GetDepartmentResources`) and all associated messages (`DeptRoleAssignment`, `SetDepartmentResourcesRequest/Response`, `GetDepartmentResourcesRequest/Response`, `AssignUserDepartmentRolesRequest`, `RemoveUserDepartmentRolesRequest`, `GetUserDepartmentRolesRequest`, `UserDepartmentRolesResponse`, `DeptRoleDetail`)
- `auth.rs` handler: remove `switch_department` method
- `department.rs` handler: remove 5 scoped-role handler methods and helpers (`strings_to_resources`, `to_dept_roles`)
- Keep basic department CRUD handlers unchanged

**Patterns to follow:**
- Proto compilation happens via `abt-grpc/build.rs` — after proto changes, `cargo build` regenerates generated code

**Test scenarios:**
- Integration: `cargo build -p abt-grpc` succeeds after proto changes
- Integration: remaining department CRUD handlers still compile and work

**Verification:**
- `cargo build -p abt-grpc` succeeds
- Generated code no longer contains removed RPC stubs

---

- [ ] **Unit 5: Service & Repository Cleanup** (R9, R10, R11, R17, R19)

**Goal:** Remove unused services, repositories, cache, and module exports

**Requirements:** R9, R10, R11, R17, R19

**Dependencies:** Unit 4

**Files:**
- Delete: `abt/src/repositories/department_resource_access_repo.rs`
- Delete: `abt/src/repositories/user_department_role_repo.rs`
- Delete: `abt/src/models/dept_role.rs` (verify no consumers first)
- Modify: `abt/src/repositories/mod.rs` — remove deleted module exports
- Modify: `abt/src/models/mod.rs` — remove `dept_role` module export
- Modify: `abt/src/permission_cache.rs` — remove `DeptResourceAccessCache` struct and singleton
- Modify: `abt/src/lib.rs` — remove `DeptResourceAccessCache` export, `get_dept_resource_access_cache()`, and startup cache load
- Modify: `abt/src/service/department_service.rs` — remove 5 scoped-role method signatures
- Modify: `abt/src/implt/department_service_impl.rs` — remove 5 scoped-role implementations and `UserDepartmentRoleRepo` import
- Modify: `abt/src/implt/permission_service_impl.rs` — remove `DepartmentResourceAccessRepo` import and department filtering logic; simplify `get_user_permissions` and `check_permission` to query `user_roles` directly
- Modify: `abt-grpc/src/permissions/mod.rs` — remove `abt::get_dept_resource_access_cache()` import

**Approach:**
- Delete entire repository files that are no longer referenced
- Remove cache struct, its singleton getter, and startup initialization
- Update module exports to remove deleted modules
- Verify `PermissionRepo::check_permission` and `PermissionRepo::get_user_permission_codes` still compile (they query `user_roles` directly and don't depend on removed code)

**Test scenarios:**
- Test expectation: none — this is removal of dead code. Verification is that `cargo build` and `cargo test` succeed

**Verification:**
- `cargo build` succeeds with no warnings about unused imports
- `cargo test` passes
- No references to deleted files remain in the codebase

---

- [ ] **Unit 6: Database Migration** (R8, R15, R16)

**Goal:** Migrate data from `user_department_roles` to `user_roles`, drop unused tables

**Requirements:** R8, R15

**Dependencies:** Unit 5 (code no longer references dropped tables)

**Files:**
- Create: `abt/migrations/018_simplify_to_global_roles.sql`

**Approach:**
1. Clear stale data in `user_roles` (leftover from pre-scoped-roles era)
2. INSERT into `user_roles` from `user_department_roles`: `SELECT DISTINCT user_id, role_id FROM user_department_roles ON CONFLICT (user_id, role_id) DO NOTHING`
3. Drop `user_department_roles` table
4. Drop `department_resource_access` table
5. Optionally: remove `parent_role_id` column from `roles` table if no longer needed

**Patterns to follow:**
- Existing migration numbering convention (sequential integers)
- Use `IF EXISTS` for defensive drops

**Test scenarios:**
- Integration: `cargo test -p abt` runs migrations and passes
- Verify data integrity: users who had roles in `user_department_roles` still have those roles in `user_roles`

**Verification:**
- Migration applies cleanly on top of existing schema
- `cargo test` passes with new migration

## System-Wide Impact

- **Interaction graph:** All 104 `require_permission`-annotated handlers are affected by macro change — but only at the generated code level, not at the call-site level
- **Error propagation:** Permission denied errors change from dept-specific messages ("User does not belong to the specified department") to role-based messages ("No permission for resource:action")
- **State lifecycle risks:** Deployment requires all users to re-login (R16). Plan for brief period where no users have valid sessions
- **API surface parity:** Frontend must stop calling `switch_department` and scoped-role department endpoints
- **Integration coverage:** `PermissionRepo::check_permission` (old path) and `check_permission_for_resource` (new path) must agree on permission decisions
- **Unchanged invariants:** Department CRUD APIs, user-department membership, role-permission management, super_admin behavior

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| All users logged out on deploy | Acceptable per R16; coordinate deploy timing |
| Role union migration widens permissions for multi-dept users | Verify with production data; documented in Dependencies/Assumptions |
| Frontend breaks on removed endpoints | Confirm frontend consumers before deploy; proto removal is a breaking change |
| `PermissionRepo` methods still query old schema | These query `user_roles` which is being kept; verify at implementation time |
| Stale data in `user_roles` from pre-scoped-roles era | Migration step 1: TRUNCATE before INSERT |

## Documentation / Operational Notes

- Coordinate deploy with frontend team to align on endpoint removal
- After deploy: verify `user_roles` table has correct data for all users
- JWT expiration determines how long old tokens circulate — with forced re-login this is immediate

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-17-simplify-permission-requirements.md](docs/brainstorms/2026-04-17-simplify-permission-requirements.md)
- **Ideation document:** [docs/ideation/2026-04-17-department-context-ideation.md](docs/ideation/2026-04-17-department-context-ideation.md)
- **Related macro docs:** [docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md](docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md)
- **Related proto enum docs:** [docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md](docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md)
