# Permission System High-Leverage Improvement Ideas

**Generated**: 2026-04-17
**Context**: ABT global RBAC system with 104 `require_permission` call sites, in-memory cache, proto-driven permission definitions, zero authorization tests, and upcoming frontend integration.

---

## 1. Frontend Permission Map via Proto Enum Reflection

**Summary**: Add a gRPC endpoint that returns a structured map of all permissions derived from the proto `Resource` and `Action` enums, enabling frontend to pre-hide unauthorized UI elements.

**Why it matters**: The frontend currently discovers permissions only through 403 errors. This endpoint would transform the proto enums (already the source of truth) into a consumable permission matrix, eliminating the "403 carpet" UX problem and reducing failed API calls by an estimated 60-80%.

**Evidence**:
- `proto/abt/v1/permissions.proto` defines `Resource` and `Action` enums
- `abt-grpc/src/permissions/mod.rs` has `PermissionCode` trait that already bridges proto enums to runtime strings
- `abt/src/models/resources.rs` has a `collect_all_resources()` function that duplicates this data statically
- Frontend team is consuming the gRPC API and needs permission visibility

**Implementation**: Add `GetPermissionMatrix` RPC that returns `map<resource_code, repeated<action_code>>` by iterating over proto enum variants via `strum::EnumIter` or manual registration. The `PermissionCode` trait already provides the `code()` conversion logic.

**Boldness**: Low — pure additive feature, no changes to existing authorization flow

**Compounding effect**: Every new frontend feature gets instant permission awareness. Future permission changes (new resources/actions) automatically propagate to frontend without UI code changes.

---

## 2. Permission Check Test Suite Generator

**Summary**: Create a compile-time test generator macro that produces authorization test cases for every `require_permission` call site, covering super_admin, normal user, empty roles, and system vs business resource branches.

**Why it matters**: The documented solution explicitly calls out "zero tests for permission check functions" as a prevention gap. A macro-based test generator would create 100% coverage of authorization branches, preventing the exact class of bugs already encountered (fail-open patterns, empty role bypasses, wrong resource type dispatch).

**Evidence**:
- `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md` prevention section: "Add permission check tests — the review found zero tests for `check_permission_for_resource`, `check_system_permission`, and `check_business_permission`"
- `abt-macros/src/lib.rs` already has the infrastructure to parse `require_permission` attributes
- Test generation would cover branches that manual testing consistently misses

**Implementation**: Extend `abt-macros` with a `#[test_permissions]` attribute that:
1. Collects all `require_permission` call sites in the module
2. Generates test cases for each (super_admin bypass, normal user with/without role, empty roles)
3. Uses `golden_file` testing to prevent regression — failing tests show exactly which permission checks changed

**Boldness**: Medium — requires macro hygiene and test harness setup, but no runtime changes

**Compounding effect**: Every new permission-checked endpoint gets free test coverage. Future refactors (scoped roles, multi-tenancy) won't introduce authorization bypasses because tests will fail immediately.

---

## 3. Single Source of Truth: Eliminate Dual-Path Permission Definitions

**Summary**: Remove the static `RESOURCES` array in `abt/src/models/resources.rs` and derive all permission metadata directly from proto enums via a build script or procedural macro.

**Why it matters**: The system currently maintains two parallel permission definitions: proto `Resource`/`Action` enums and the `RESOURCES` static array. This violates DRY and has already caused drift issues (proto enum migration required manual updates). Consolidating to a single source eliminates an entire class of "forgot to update X" bugs.

**Evidence**:
- `abt/src/models/resources.rs` defines `RESOURCES` static array with Chinese resource names
- `proto/abt/v1/permissions.proto` defines `Resource` and `Action` enums
- `abt-grpc/src/permissions/mod.rs` has `PermissionCode` trait that manually maps proto variants to strings
- `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md` documents the migration from strings to proto enums — exactly the kind of drift this prevents

**Implementation**: Use a `build.rs` script or derive macro to:
1. Parse proto enum definitions at compile time
2. Generate `RESOURCES` array with `resource_name`/`description` from proto comments or custom attributes
3. Auto-generate `PermissionCode` trait impls

**Boldness**: Medium — changes build process and requires proto comment conventions

**Compounding effect**: Every new permission added to proto automatically appears in `collect_all_resources()`, permission checks, and frontend permission maps. No more "add to proto, update array, update trait" three-step process.

---

## 4. Cache Invalidation Trigger via Database Notifications

**Summary**: Add PostgreSQL `LISTEN/NOTIFY` support for role/permission changes, eliminating the manual `refresh()` call and preventing stale permission data without full cache reloads.

**Why it matters**: The current cache requires manual refresh when permissions change, which is error-prone and causes either stale permissions (if forgotten) or unnecessary full reloads (if over-used). Database notifications would invalidate only affected roles, reducing cache update overhead by ~90% for granular changes.

**Evidence**:
- `abt/src/permission_cache.rs` uses `parking_lot::RwLock` for concurrent access
- Cache is loaded at startup via `load()` with full database scan
- SQLx already has PgListener support for notifications
- Role/permission changes are infrequent but security-critical

**Implementation**:
1. Add triggers on `role_permissions` and `roles` tables to emit `NOTIFY role_permission_changed`
2. Spawn a background task in `abt-grpc` that listens and calls selective cache invalidation
3. Extend `RolePermissionCache` with `invalidate_role(role_id)` method that recomputes only that role's permissions

**Boldness**: Medium — requires database triggers and background task management

**Compounding effect**: Permission changes take effect immediately without deployment. Future features like real-time audit dashboards can tap into the same notification stream.

---

## 5. Permission Check Metrics Middleware

**Summary**: Add a lightweight metrics collector to `require_permission` macro that tracks authorization outcomes (allow/deny) by resource:action, exposing denial rates for security monitoring and permission optimization.

**Why it matters**: Zero visibility into permission denials means you can't detect misconfigured roles or permission creep. A denial spike indicates either a bug (fail-open reversion) or a user workflow problem. Metrics would turn authorization into an observable system rather than a black box.

**Evidence**:
- `require_permission` macro is already generating code at all 104 call sites — adding metrics is a single macro change
- Production system needs observability for security-relevant operations
- No existing metrics on authorization outcomes

**Implementation**:
1. Add `permission_check_outcome{resource,action,result}` counter to the macro expansion
2. Expose via Prometheus endpoint (already standard in gRPC servers)
3. Add dashboard showing denial rates by resource

**Boldness**: Low — pure additive, no behavior changes

**Compounding effect**: Every permission check contributes to security observability. Misconfigured roles are detected via denial spikes instead of user complaints. Future audit features can query the same metrics.

---

## 6. JIT Permission Cache Loading with Fail-Closed Grace Period

**Summary**: Replace startup cache load with just-in-time loading per-role, with a 5-minute grace period that allows existing JWTs to expire while preventing new logins with stale permissions.

**Why it matters**: Current fail-closed startup (`expect` on cache load) blocks all service starts if the permission table is unreachable. A hybrid approach — allow service starts but deny new auth/permission checks — provides better availability during database blips while maintaining security guarantees.

**Evidence**:
- `abt/src/lib.rs:108-111` has hard fail-closed on cache load
- JWT expiration is 1 hour — new auth attempts during outage would fail anyway
- `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md` documents the fail-open vulnerability

**Implementation**:
1. Change cache load to lazy: `get_role_permissions(role_id)` loads from DB if not in cache
2. Add background refresh task that loads cache within 5 minutes of startup
3. Deny new login attempts during grace period, allow existing JWTs to continue

**Boldness**: High — changes availability semantics during database outages

**Compounding effect**: Database blips no longer block deployments. Future multi-region setups can tolerate regional database failures without full service disruption.

---

## 7. Role Inheritance Visualization via Graph Export

**Summary**: Add a `GetRoleGraph` gRPC endpoint that returns the role inheritance tree as a DOT/Graphviz format, enabling role designers to detect circular dependencies and optimize permission structures.

**Why it matters**: The cache already has cycle detection logic (`detect_cycles()` in `permission_cache.rs`), but that only prevents crashes — it doesn't help role designers understand complex inheritance chains. A visualization export would turn the implicit graph into an explicit design tool.

**Evidence**:
- `abt/src/permission_cache.rs:114-149` has DFS cycle detection that traverses parent relationships
- `roles` table has `parent_role_id` for inheritance
- Zero visibility into role graph structure for administrators

**Implementation**:
1. Add `GetRoleGraph` RPC that queries `roles` table
2. Return DOT format string: `digraph roles { "admin" -> "moderator"; }`
3. Frontend can render with graphviz libraries or download as file

**Boldness**: Low — pure additive feature

**Compounding effect**: Role refactoring becomes data-driven. Future features like "suggest role merge" can analyze the graph structure. Circular dependency detection becomes visible rather than just a crash prevention mechanism.

---

## 8. Permission Audit Log with Enrichment

**Summary**: Add an audit log table that records all permission denials with context (user_id, resource, action, role_ids, timestamp), enabling compliance reporting and security investigation.

**Why it matters**: Current system has no record of denied permission attempts. This makes compliance audits ("who tried to access X and was denied?") impossible and security investigations ("was this user probing for permissions?") require manual log parsing. An enriched audit log turns authorization into a queryable event stream.

**Evidence**:
- `abt-grpc/src/handlers/permission.rs:136-144` has `list_audit_logs` but it's for permission changes, not access denials
- Compliance requirements for BOM/inventory systems typically include access audit trails
- `require_permission` macro is the perfect injection point for denial logging

**Implementation**:
1. Add `permission_denial_logs` table with columns: user_id, resource, action, role_ids, timestamp, request_id
2. Extend `require_permission` macro to emit denial event (allow flag to disable for performance)
3. Add `QueryPermissionDenials` RPC for audit export

**Boldness**: Medium — requires database table and careful performance tuning

**Compounding effect**: Every denied permission becomes a security data point. Compliance audits become one SQL query instead of log parsing. Future ML-based anomaly detection can feed on the denial stream.

---

## 9. Permission Check Caching via Computed Hash

**Summary**: Add a `permission_hash` field to JWT claims that encodes the user's role permissions at login time, allowing per-request cache lookups without hitting the role_permission table or the in-memory cache.

**Why it matters**: Every request currently traverses the role inheritance chain via cache lookups. For high-traffic endpoints (inventory updates, price changes), this is unnecessary work. A pre-computed permission hash would turn authorization into a constant-time hash comparison.

**Evidence**:
- `abt/src/models/auth.rs` has `Claims` struct with `role_ids` field
- `abt-grpc/src/interceptors/auth.rs` decodes JWT on every request
- Role inheritance resolution is recursive (`resolve_permissions()` in cache)

**Implementation**:
1. At login time, compute `SHA256(role_ids || permission_codes)` and add to JWT
2. Add `permission_cache` that maps `hash -> HashSet<permission_codes>`
3. Change `require_permission` to check hash instead of role_ids
4. Invalidate hash when permissions change (ties into #4 database notifications)

**Boldness**: High — changes JWT format and cache key strategy

**Compounding effect**: Authorization becomes ~10x faster for cached users. Future features like "permission downgrade without logout" become trivial — just update the hash-to-permissions mapping. High-traffic endpoints see measurable latency improvements.

---

## 10. Declarative Permission Tests via YAML

**Summary**: Add a YAML-based permission test suite that defines "role X should have permissions Y" and generates integration tests, allowing non-developers to verify permission models without writing Rust code.

**Why it matters**: Permission requirements are business logic ("warehouse managers should edit inventory but not delete products"), not implementation details. Currently, verifying these requirements requires writing Rust tests or manual API calls. A declarative test format would allow QA/business teams to own permission validation.

**Evidence**:
- Zero permission tests exist today (per security solution doc)
- Business teams define permission matrices, not developers
- Proto already uses declarative definitions

**Implementation**:
1. Define YAML schema:
   ```yaml
   tests:
     - role: warehouse_manager
       should_have: [inventory:write, inventory:read, product:read]
       should_not_have: [product:delete, role:write]
   ```
2. Build script parses YAML and generates Rust integration tests
3. Tests call `check_permission_for_resource` directly

**Boldness**: Low — pure additive, separate crate

**Compounding effect**: Permission validation becomes part of the QA process. Business logic changes ("give warehouse managers product delete rights") are testable without code changes. Future role redesigns can be validated in CI before deployment.

---

## Leverage Analysis Summary

| Idea | Blast Radius | Time Investment | Compounding Effect |
|------|--------------|-----------------|-------------------|
| 1. Frontend Permission Map | All UI/UX | 2 days | Eliminates 403 errors permanently |
| 2. Test Generator Macro | All authorization | 3 days | Prevents bypass bugs forever |
| 3. Single Source of Truth | All definitions | 2 days | Eliminates drift class |
| 4. Cache Invalidation | All permission changes | 3 days | Real-time permissions + observability |
| 5. Metrics Middleware | All checks | 1 day | Security observability baseline |
| 6. JIT Cache Loading | Service availability | 4 days | Deployment resilience |
| 7. Role Graph Export | Role design | 1 day | Visualization for future features |
| 8. Audit Log | Compliance/Security | 3 days | Audit queryability forever |
| 9. Permission Hash | Performance | 5 days | 10x auth performance |
| 10. YAML Tests | QA process | 2 days | Business-owned validation |

**Top 3 by leverage**:
1. **Idea #2 (Test Generator)** — highest security ROI, prevents the exact bugs already encountered
2. **Idea #1 (Frontend Map)** — highest UX ROI, eliminates a whole class of user-facing errors
3. **Idea #3 (Single Source)** — highest maintenance ROI, kills the drift bug pattern forever

**Quick wins** (< 2 days): #5 (Metrics), #7 (Graph Export), #10 (YAML Tests)

**Strategic bets**: #4 (Cache Invalidation), #8 (Audit Log), #9 (Permission Hash)

---

## Prioritization Matrix

```
High Impact, Low Effort (Do First):
├── #5 Metrics Middleware (1 day)
└── #7 Role Graph Export (1 day)

High Impact, Medium Effort (Do Soon):
├── #1 Frontend Permission Map (2 days)
├── #2 Test Generator Macro (3 days)
├── #3 Single Source of Truth (2 days)
└── #10 YAML Tests (2 days)

High Impact, High Effort (Plan For):
├── #4 Cache Invalidation (3 days)
├── #8 Audit Log (3 days)
└── #9 Permission Hash (5 days)

Medium Impact, Medium Effort (Defer):
└── #6 JIT Cache Loading (4 days)
```

**Recommended sprint sequence**:
1. **Sprint 1**: Metrics (#5) + Graph Export (#7) — establish observability baseline
2. **Sprint 2**: Frontend Map (#1) + YAML Tests (#10) — empower frontend + QA teams
3. **Sprint 3**: Test Generator (#2) + Single Source (#3) — eliminate drift and bypass bugs
4. **Sprint 4+**: Cache Invalidation (#4) + Audit Log (#8) + Permission Hash (#9) — performance + compliance
