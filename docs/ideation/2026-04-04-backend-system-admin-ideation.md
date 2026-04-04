---
date: 2026-04-04
topic: backend-system-admin-redesign
focus: E:\work\front\abt_front\docs\plans\2026-04-04-backend-requirements.md
---

# Ideation: Backend System Admin Module Redesign

## Codebase Context

**Project:** ABT — BOM and Inventory Management System. Rust workspace with 3 crates (common, abt core lib, abt-grpc server) + proto definitions. gRPC via tonic, PostgreSQL via sqlx (compile-time checked).

**Key observations:**
- 14 proto services, consistent 4-layer architecture (proto → model → repo → service → handler)
- `PaginationParams`/`PaginationInfo` defined in `base.proto` but used by **zero** RPCs — all List RPCs return unpaginated or ad-hoc paginated results
- Auth interceptor extracts JWT into `AuthContext` but does NOT check permissions — 101 manual `check_permission()` calls across 12 handler files
- Departments exist in schema with CRUD + user assignment, but **no row-level visibility filtering** implemented anywhere
- `UserResponse` lacks departments; `RoleListResponse` uses separate type without permissions
- N+1 query pattern in `ListUsers` (loops `get_user_roles` per user)
- Excel import progress broken (OnceLock singleton loses state across instances/restarts)
- File upload handler has no filename sanitization, no size limit, no temp file cleanup

**Past learnings:**
- Auth implementation plan (APPROVED) at `docs/superpowers/plans/2026-04-01-auth-implementation.md` chose manual `check_permission()` in handlers over interceptor-level RBAC
- Department design at `docs/superpowers/specs/2026-03-31-department-design.md` specifies flat structure, orthogonal to roles
- RBAC simplified from 3-table model to `role_permissions(resource_code, action_code)`
- sqlx compile-time checks require live DB with all migrations applied
- No pagination documentation exists yet

## Ranked Ideas

### 1. Declarative gRPC RBAC Interceptor Pipeline
**Description:** Replace 101 manual `extract_auth()` + `check_permission()` calls with a layered interceptor pipeline: Layer 1 validates JWT (current), Layer 2 checks method-level permissions via static map from gRPC method path to `resource:action`, Layer 3 resolves department scope. Permission mapping becomes a single declarative table. Future: audit logging as Layer 4.
**Rationale:** Eliminates entire class of security bugs (forgotten check on new RPC). Reduces handler code ~30%. Infrastructure already exists (AuthContext, check_permission, RESOURCES array).
**Downsides:** Reverses a decision in the approved auth plan. Requires careful mapping of all 50+ methods. Some RPCs (Login) must be excluded.
**Confidence:** 90%
**Complexity:** Medium
**Status:** Explored (brainstorm 2026-04-04)

### 2. Standardized List RPC Pattern (Pagination + Filtering + Envelope) (Pagination + Filtering + Envelope)
**Description:** Activate dormant `PaginationParams`/`PaginationInfo` in `base.proto` for ALL List RPCs. Consistent request shape (pagination + keyword + entity filters). Consistent response shape (items + PaginationInfo + total). Covers ListUsers, ListRoles, ListDepartments, ListAuditLogs (P0) and harmonizes existing ad-hoc pagination.
**Rationale:** 15 list endpoints each invent their own shape. One pattern = one frontend pagination contract. base.proto types exist but are dead code.
**Downsides:** Proto changes cascade to all clients. Audit logs need date-range and target-type filters beyond basic pagination.
**Confidence:** 95%
**Complexity:** Medium
**Status:** Unexplored

### 3. Department-Based Data Isolation
**Description:** Make departments functional by: (a) adding `department_id` to business entity tables, (b) embedding department IDs in JWT/AuthContext, (c) injecting department-scoped WHERE clauses at repository layer, (d) DepartmentVisibilityService for explicit resource allowlists.
**Rationale:** Departments are cosmetic — any user with `product:read` sees ALL products. P1 requirement explicitly asks for this.
**Downsides:** Requires migrations on all business tables. Design decisions around null department_id semantics. Allowlist model needs careful design for unassigned resources.
**Confidence:** 80%
**Complexity:** High
**Status:** Explored (brainstorm 2026-04-04, requirements doc: docs/brainstorms/2026-04-04-department-isolation-requirements.md)


**Description:** Add `repeated DepartmentInfo departments` to `UserResponse`. Add `repeated string permission_codes` to `RoleListItem`. Admin UI shows full profile in one call instead of 3.
**Rationale:** P2 requirement. Small scope, high value. Data already exists — model just needs extension.
**Downsides:** Slightly larger responses. List queries need JOINs for department/permission data.
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 5. N+1 Query Elimination in List Operations
**Description:** Replace sequential `for user in users { get_user_roles() }` loop with single JOIN/batch query. Add input size limits to BatchAssignRoles (cross-product can overflow PostgreSQL 65535 bind-parameter limit).
**Rationale:** N+1 in ListUsers confirmed at `user_service_impl.rs:82-89`. BatchAssignRoles cross-product at `user_repo.rs:199-207` is unguarded crash vector (256x256 = overflow).
**Downsides:** Requires rewriting list implementations. JOIN queries slightly more complex with sqlx.
**Confidence:** 90%
**Complexity:** Medium
**Status:** Unexplored

### 6. File Upload Security Hardening
**Description:** Fix Excel upload handler: sanitize filenames, enforce max file size, validate file type, clean up temp files after import.
**Rationale:** Client-supplied filename used directly in path concatenation (path traversal), no size limit on stream (DoS), no temp file cleanup (disk exhaustion).
**Downsides:** May slightly complicate upload flow. Needs careful testing.
**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | JWT Permission Architecture for Scale | Premature — ~14 services work fine with embedded permissions |
| 2 | Transaction Helper Macro | Claimed 30+ sites not grounded; pattern manageable at current scale |
| 3 | Service Instance Caching | Arc<PgPool> clone is cheap; no measurable benefit |
| 4 | Department as Closure Table | Speculative — flat departments are stated design; YAGNI |
| 5 | Tag-Based Visibility | No tag system exists; explicit allowlists are the stated requirement |
| 6 | Proto Field Mask Enrichment | Over-engineering for internal system with few consumers |
| 7 | Codegen Handlers from Proto | Negative ROI at 14 services; hard-to-debug generated code |
| 8 | sqlx Runtime/Compile-Time Split | Defeats purpose of compile-time checks; not a real problem here |
| 9 | Derive Permissions from Reflection | Weaker version of declarative interceptor; naming conventions fragile |
| 10 | Generic Excel Import Framework | Only one import service exists; premature abstraction |
| 11 | AppState Service Registry | Current factory pattern works; underspecified alternative |

## Bonus Bug Discoveries

- `warehouse_id: 0` hardcoded in 6 mapping blocks in `inventory.rs` — confirmed data-integrity bug
- Excel import `OnceLock` singleton loses state across server restarts — needs DB-backed progress

## Session Log
- 2026-04-04: Initial ideation — 48 raw ideas generated (6 agents × ~8 ideas), 25 unique after dedup, 6 survived adversarial filtering. Brainstorm completed for ideas #1 (RBAC macro) and #3 (department isolation).
- 2026-04-04: Brainstorm #1 → requirements doc at `docs/brainstorms/2026-04-04-rbac-interceptor-macro-requirements.md`
- 2026-04-04: Brainstorm #3 → requirements doc at `docs/brainstorms/2026-04-04-department-isolation-requirements.md`

