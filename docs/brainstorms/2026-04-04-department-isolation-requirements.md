---
date: 2026-04-04
topic: department-data-isolation
---

# Department-Based Data Isolation (v2)

## Problem Frame

系统已有 RBAC 权限控制（谁能做什么操作），但缺少部门级数据隔离（谁能看到哪些资源类型）。当前任何有 `product:read` 权限的用户能看到所有产品，无法实现部门级可见性控制。

前版方案（v1）提议在业务表上加 `department_id` 列，但这有两个问题：(1) 一个资源只能属于一个部门，不支持多部门共享；(2) 部门能访问产品表就应该能看全部产品，不需要逐行限制。

新方案采用 **关联表 + JWT 裁剪** 模式：用关联表定义部门能访问的资源类型（表级别），在登录时裁剪 JWT 权限，下游零改动。

## Requirements

**Department-Resource Access Data Model**
- R1. Create `department_resource_access(department_id, resource_code)` junction table with composite primary key and a foreign key on `department_id` referencing `departments(department_id)`, where `resource_code` is VARCHAR matching the existing resource code convention (same as `role_permissions.resource_code`)
- R2. If a department has a row for `resource_code = 'product'`, all users in that department can see ALL products (table-level visibility, not row-level)
- R3. Only business resource types are subject to department isolation: `product`, `term`, `bom`, `warehouse`, `location`, `inventory`, `price`, `labor_process` (8 types)
- R4. System resource types bypass department filtering entirely: `user`, `role`, `permission`, `department`, `excel` (5 types). Users with role-granted permissions for these resources always retain access regardless of department membership
- R2b. A department with zero rows in `department_resource_access` grants access to NO business resources (fail-closed). Users in such a department can only access system resources (if their role permits)

**Default Department**
- R5. Add `is_default BOOLEAN NOT NULL DEFAULT false` column to `departments` table. Exactly one department should be marked as default. The enforcement mechanism (DB constraint vs application level) is deferred to planning
- R5b. If no default department exists at permission resolution time (or the default department is inactive), treat the user as having no department membership (fail-closed: no business resource access)
- R6. Users with no department assignment are automatically treated as members of the default department for permission resolution purposes (no implicit `user_departments` row is created — the default department is resolved at login time). The default department's resource access is then checked per R2b — if the default department has zero configured resources, the user has no business resource access
- R7. Administrators can configure the default department's accessible resources through the same management API as other departments

**Permission Resolution (Login-Time Pruning)**
- R8. Modify BOTH `AuthRepo::get_user_permission_codes()` (JWT construction path) AND `PermissionRepo::get_user_permission_codes()` (permission API path) to apply department filtering: resolve the user's effective departments (explicit memberships, or default department if none), query `department_resource_access` for accessible resource codes (union of all departments' accessible resource codes), then return only permissions where either (a) the resource_code is a system resource, or (b) the resource_code is in the user's department-accessible set. A user in multiple departments gets the union of all their departments' accessible resources
- R9. `super_admin` bypass: users with `is_super_admin = true` receive an empty permissions list in the JWT; `AuthContext::check_permission` short-circuits on `is_super_admin=true`. No department filtering is needed for super_admin users (existing behavior, unchanged)
- R10. Department filtering applies in `login()`, `refresh_token()`, and `get_user_claims()` — all three code paths that construct `Claims`

**Management API**
- R11. Provide gRPC RPCs for department resource access management: `SetDepartmentResources(department_id, resource_codes[])` with full-overwrite semantics (replaces existing), `GetDepartmentResources(department_id)` returns current list
- R11b. `SetDepartmentResources` must validate each `resource_code` against the known business resource types list (8 types from R3). Reject the entire request if any value is unrecognized. System resource codes (R4) should be silently ignored (not stored)
- R11c. `SetDepartmentResources` must validate that `department_id` references an existing, active department
- R12. Only users with `department:write` permission can call `SetDepartmentResources`. Only users with `department:read` permission can call `GetDepartmentResources`. These align with the existing department management permission model
- R13. The default department's resources can be modified through the same RPCs as other departments. The existing `delete_department` RPC must reject deletion of the department where `is_default=true`

**Migration**
- R15. The migration must seed a default department with `is_default=true` and assign it all 8 business resource codes. This ensures existing users without department assignments retain access to business data after deployment
- R16. The migration must seed `department_resource_access` with all 8 business resource codes for every existing department. This prevents a breaking change where all users lose business data access on deployment
- R16b. R15 and R16 seeding must be atomic within a single database transaction. If the migration cannot complete, it must roll back entirely, leaving the system in its pre-migration state

**Cleanup**
- R14. Remove the `department_id` field from the Rust `Resource` model struct in `abt/src/models/permission.rs` — a dead artifact from the `resources` table that was dropped in migration 015. No database migration is needed since the table no longer exists

## Success Criteria

- Administrator configures "Electronics" department with access to `product` and `inventory`. Users in Electronics with role granting `product:read` see ALL products. Users in Electronics with role granting `bom:read` see nothing (department doesn't allow `bom`)
- User in "Electronics" (product+inventory) AND "Production" (product+bom+labor_process) sees products, inventory, BOMs, and labor processes — the union of both departments' accessible resources
- User with NO department assignment, default department configured with `product` only: user can access products (if role grants it) but not BOMs
- Super admin sees everything regardless of department membership or department resource access configuration
- System resources (user, role, permission, department, excel) are always accessible to users with the corresponding role permissions, regardless of department configuration
- Existing `AuthContext::check_permission()` behavior is unchanged — the pruning happens upstream at JWT construction time
- Fresh deployment with no departments configured: all users fall back to default department with all 8 business resources (migration seeding), existing behavior preserved
- Newly created department with no resource access configured: users in that department see no business data (fail-closed)

## Scope Boundaries

- **Not doing**: Row-level data isolation (within a resource type, all or nothing — table-level visibility only)
- **Not doing**: Cross-department data sharing ("share this product with department B")
- **Not doing**: Hierarchical departments (flat structure)
- **Not doing**: Audit logging for department resource access changes (separate concern)
- **Not doing**: Modifying `AuthContext::check_permission()` logic
- **Not doing**: Adding `department_id` columns to any business entity table
- **Not doing**: Token revocation or force-logout when department access changes (re-login within JWT TTL is accepted)
- **Not in scope now**: Tag-based or attribute-based visibility (future consideration)

## Key Decisions

- **Association model**: Junction table `department_resource_access` instead of `department_id` column on business tables. Reason: supports M:N, matches the "table-level access" requirement, consistent with existing `role_permissions` pattern
- **Pruning point**: JWT construction time (login/refresh). Reason: zero downstream code changes, `AuthContext` and all handlers remain untouched
- **Re-login requirement**: Department resource access changes require re-login to take effect. The maximum stale-access window equals `jwt_expiration_hours`. This is accepted as a tradeoff for zero runtime overhead. If faster propagation is needed in the future, a token revocation mechanism can be added separately
- **Default department**: `is_default` flag on `departments` table. Reason: configurable by admin through normal UI, consistent with existing data model
- **System resources excluded**: Department isolation only applies to business data. Reason: admin functions like user/role/permission management should not be blocked by department membership
- **Fail-closed semantics**: A department with zero configured resource access grants no business data visibility. This is safer than fail-open
- **Both permission paths filtered**: Both AuthRepo (JWT) and PermissionRepo (API) must apply department filtering to prevent inconsistency between what users can do and what the permission API reports

## Dependencies / Assumptions

- `user_departments` table (migration 012) provides user-department membership
- `DepartmentRepo::get_user_department_ids()` already exists for querying user's departments
- `role_permissions` table (migration 015) uses `resource_code VARCHAR(128)` — the same string format is used in the junction table
- `AuthServiceImpl` is the sole constructor of JWT Claims, making it the single point of change for permission pruning
- The `resources` table was dropped entirely in migration 015 (including the `department_id` column added in migration 012). The Rust `Resource` model struct still carries a `department_id` field that should be cleaned up
- The `excel` resource type is classified as a system resource. Excel import can create/modify business data (products, BOMs). Since department isolation is table-level (not row-level), imported data is visible to ALL departments that have access to that resource type. This is an accepted tradeoff of table-level isolation

## Outstanding Questions

### Deferred to Planning
- [Affects R1][Technical] Migration strategy for the new `department_resource_access` table and `departments.is_default` column
- [Affects R5][Technical] Whether to enforce "exactly one default department" at the DB level (partial index/exclude constraint) or application level
- [Affects R8][Technical] How to structure the department filtering logic — whether to add raw SQL to AuthRepo, inject a DepartmentRepo dependency, or create a new coordination layer
- [Affects R11][Technical] Proto definition: add RPCs to existing `department.proto` or create a new `department_resource_access.proto`
- [Affects R14][Technical] Clean up Rust `Resource` model struct — verify no code references to `department_id` field exist before removing

## Next Steps
→ `/ce:plan` for structured implementation planning
