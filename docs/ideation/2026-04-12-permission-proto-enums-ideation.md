---
date: 2026-04-12
topic: permission-proto-enums
focus: Move permission definitions from hardcoded strings into proto so both frontend and backend can use generated code
---

# Ideation: Proto-Defined Permission Enums

## Codebase Context

**Project:** ABT — Rust gRPC BOM/Inventory system with PostgreSQL backend.
**Current state:** 101 handler methods across 12 handler files use `#[require_permission("resource", "action")]` with string literals. The macro validates only arg count (must be 2), not content. The canonical list of 13 resources and 3 actions lives in `abt/src/models/resources.rs` as a 77-line hand-maintained static array. Proto has `ResourceInfo`/`PermissionInfo` messages with bare `string` fields. Frontend has no compile-time access to permission constants — must call `ListPermissions` RPC at runtime.

**Pain points:** Typos in permission strings compile fine and fail silently at runtime. Three disconnected definition sites (proto strings, `resources.rs`, macro annotations). Frontend lacks type-safe permission values.

**Leverage points:** `tonic_prost_build` already auto-generates Rust code from proto. Proto compilation pipeline exists and is well-tested. Only 13 resources and 3 actions — small, bounded domain.

## Ranked Ideas

### 1. Define Resource and Action as Proto Enums
**Description:** Add `enum Resource { PRODUCT=0; TERM=1; BOM=2; WAREHOUSE=3; LOCATION=4; INVENTORY=5; PRICE=6; LABOR_PROCESS=7; USER=8; ROLE=9; PERMISSION=10; DEPARTMENT=11; EXCEL=12; }` and `enum Action { READ=0; WRITE=1; DELETE=2; }` to `permission.proto`. `tonic_prost_build` auto-generates Rust enums.
**Rationale:** Keystone change — proto becomes single source of truth for permission vocabulary.
**Downsides:** Proto enums can't carry Chinese display names directly; need sidecar mapping or custom options.
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 2. Auto-Generate resources.rs from Proto Enums
**Description:** Delete the hand-maintained `RESOURCES` static array. Generate `ResourceActionDef` entries at build time from proto enums. Display names and `BUSINESS_RESOURCE_CODES`/`SYSTEM_RESOURCE_CODES` derived from proto enum value options or a minimal Rust-side mapping.
**Rationale:** Eliminates three-way sync problem. One edit in proto propagates everywhere.
**Downsides:** Need to handle display names (Chinese labels) — proto enum value options, sidecar file, or small Rust mapping.
**Confidence:** 85%
**Complexity:** Medium
**Status:** Unexplored

### 3. Frontend Code Generation from Proto
**Description:** Run proto compilation for TypeScript so frontend imports typed `Resource` and `Action` enums. Frontend no longer needs `ListPermissions` RPC to discover permission vocabulary.
**Rationale:** The other half of "前后端都可以正常使用". Compile-time-safe frontend permission constants with zero network cost.
**Downsides:** Requires setting up frontend proto compilation pipeline (new tooling).
**Confidence:** 90%
**Complexity:** Low-Medium
**Status:** Unexplored

### 4. Macro Accepts Enum Paths (Compile-Time Validation)
**Description:** Change `#[require_permission("warehouse", "read")]` to `#[require_permission(Resource::Warehouse, Action::Read)]`. Macro emits tokens as-is, generating `auth.check_permission(Resource::Warehouse.as_str_name(), Action::Read.as_str_name())`. Rust compiler validates variants exist.
**Rationale:** Typo like `Resource::Warehoues` is immediate compile error across all 101 handler methods.
**Downsides:** Requires updating all 101 call sites (mechanical). Handler files need to import generated enum types.
**Confidence:** 80%
**Complexity:** Medium (mostly mechanical)
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | Compile-time string validation (backward compat) | Same infra cost as ideas 2+3, worse outcome, still stringly-typed |
| 2 | Proto method annotations + interceptor | Tonic interceptors can't do per-RPC dispatch; macro is correct for this arch |
| 3 | Structured JWT permissions | Breaking schema change for zero functional improvement on 39-element Vec |
| 4 | Bit-packed integer permission IDs | Micro-optimizing 39-element string match; destroys debuggability |
| 5 | ReBAC model | Nuclear reactor for a desk lamp; 13-resource flat model is correct |
| 6 | Database-driven permissions | Solves opposite problem; breaks compile-time guarantee |
| 7 | Audit trail auto-gen | Conflates authorization with accountability; reads aren't auditable |
| 8 | Test fixture generation | 39 identical tests catch nothing existing 5 tests don't |
| 9 | Permission manifest JSON | Inferior to proto-to-TS codegen which ecosystem supports |
| 10 | Build-time handler audit | Fragile source scanning when type system validates for free |

## Session Log
- 2026-04-12: Initial ideation — ~32 raw ideas generated across 4 frames (pain/friction, leverage, inversion/removal, reframing), deduped to 14 unique candidates, 4 survived adversarial filtering. User selected "brainstorm full migration path" as next step.
