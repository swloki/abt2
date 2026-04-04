---
date: 2026-04-04
topic: department-resource-access
focus: Redesign department-resource association model — replace department_id column approach with junction table + table-level visibility
---

# Ideation: Department-Resource Access Model

## Codebase Context

- Rust workspace with gRPC API + PostgreSQL. Departments, users-departments (M:N), RBAC with 13 resource types already exist.
- `resources` table has `department_id` column (migration 012) but it's unused — and wrong because resources has one row per type (1:1).
- Business entity tables (products, boms, warehouses, locations) have NO department fields.
- `resource_code` is the stable string identifier used everywhere: `role_permissions(resource_code, action_code)`, `AuthContext::check_permission(resource, action)`, JWT `permissions: Vec<String>`.

## User's Key Constraints

1. A resource must be accessible by multiple departments (M:N) — single `department_id` column is wrong
2. If department A can access the "products" resource type, they see ALL products — table-level, not row-level
3. Must use a proper association table, not columns on business entity tables

## Ranked Ideas

### 1. Department-Resource Type Junction Table
**Description:** Create `department_resource_access(department_id, resource_code)` table. If department has `(1, 'product')` row = can see all products. Supports M:N. Zero business table changes. Structurally identical to existing `role_permissions` pattern.
**Rationale:** Direct answer to the user's requirements. Simplest schema, clearest semantics, reuses established `resource_code` convention.
**Downsides:** New table + CRUD management API needed.
**Confidence:** 95%
**Complexity:** Low
**Status:** Selected for brainstorm

### 2. RBAC Intersection Model
**Description:** Effective permissions = role_permissions INTERSECT department_allowed_resources. Department access gates which resource types a user's role permissions apply to. No separate visibility check layer.
**Rationale:** Eliminates a parallel visibility system by folding department scope into existing permission resolution.
**Downsides:** "Intersection" semantics may confuse administrators.
**Confidence:** 85%
**Complexity:** Low-Medium
**Status:** Unexplored

### 3. Departments Own Roles
**Description:** Remove `user_roles`. Add `department_roles(department_id, role_id)`. Joining a department = getting its roles. One admin action instead of two.
**Rationale:** Most radical simplification of admin workflow.
**Downsides:** Breaking change to existing user_roles API, highest migration cost.
**Confidence:** 70%
**Complexity:** High
**Status:** Unexplored

### 4. JWT-Embedded Department Scope
**Description:** At login time, query department_resource_access, intersect with role permissions, embed only effective permissions in JWT. Zero downstream code changes.
**Rationale:** Eliminates runtime department filtering entirely. AuthContext::check_permission() unchanged.
**Downsides:** Department changes require re-login to take effect.
**Confidence:** 90%
**Complexity:** Low
**Status:** Unexplored — recommended as implementation strategy paired with idea #1

### 5. YAGNI — Rethink Whether Department Isolation Is Needed
**Description:** Existing RBAC already provides table-level access control. BOM/inventory systems are inherently cross-functional. Departments may only need organizational attribution, not data isolation.
**Rationale:** Challenges the premise. Avoids creating data silos in a domain where BOMs naturally cross department boundaries.
**Downsides:** If business truly needs isolation, this defers necessary work.
**Confidence:** 60%
**Complexity:** Zero
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | Activate resources.department_id | resources table has one row per type — 1:1 doesn't support M:N |
| 2 | PostgreSQL RLS | Row-level feature for a table-level requirement; also requires department_id on business tables |
| 3 | Add department_code to role_permissions | Conflates two concerns; makes schema harder to reason about |
| 4 | Permission string prefix (dept_id:resource:action) | String hack, fragile, hard to query |
| 5 | JSONB department_ids on business tables | User explicitly rejected adding department fields to business tables |
| 6 | JSONB accessible_resources on departments table | Functional but JSONB for simple M:N is an anti-pattern when junction table is cleaner |
| 7 | Tag-based access control | Over-engineered for current needs; YAGNI |
| 8 | Derived visibility from metadata | Too complex; fragile rule-based filtering |
| 9 | Department context switching (Slack workspace) | Changes UX model; not what was asked for |
| 10 | Audit logging reuse | Not a data model idea; supporting feature only |

## Session Log
- 2026-04-04: Initial ideation — 32 raw ideas generated across 4 frames, 18 unique after dedupe, 5 survived. Idea #1 (junction table) + #4 (JWT pruning) selected for brainstorm.
