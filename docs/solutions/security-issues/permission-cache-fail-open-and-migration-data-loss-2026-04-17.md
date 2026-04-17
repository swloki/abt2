---
title: "Fail-Open Permission Cache and Migration Data Loss in Scoped-Role Refactoring"
date: 2026-04-17
category: security-issues
module: authentication
problem_type: security_issue
component: authentication
severity: critical
symptoms:
  - "Non-super_admin users permanently locked out after cache initialization failure"
  - "eprintln warning logged but application continues with empty permission cache"
  - "Migration TRUNCATE destroys existing user_roles before inserting migrated data"
root_cause: config_error
resolution_type: code_fix
tags: [permissions, security, fail-open, cache, migration, postgres, rust, once-lock]
---

# Fail-Open Permission Cache and Migration Data Loss in Scoped-Role Refactoring

## Problem

During a refactoring that simplified the permission system from department-scoped roles to global roles, code review uncovered two P0 issues:

1. **Fail-open cache initialization**: The permission cache loaded at startup with a soft failure pattern (`eprintln!` warning). If the database was unreachable, the cache stayed empty and the system ran with no permissions — permanently locking out all non-super_admin users.

2. **Migration data loss**: Migration 018 ran `TRUNCATE user_roles` before inserting migrated role data, destroying existing global role assignments. Combined with `DROP TABLE` on source tables, the migration was irreversible after commit.

Both issues were caught during code review before reaching production.

## Symptoms

- If database is unreachable at startup, the server starts but all `has_permission()` calls return `false`
- The empty cache persists for the process lifetime due to `OnceLock` singleton semantics
- `eprintln!` warning is invisible in production (no structured logging)
- Migration `TRUNCATE` destroys data even inside a transaction — if the subsequent `INSERT` fails, the table is left empty
- `DROP TABLE` after commit has no rollback path

## What Didn't Work

### Cache Initialization

The original code treated cache loading as non-critical:

```rust
// BEFORE (dangerous — fail-open):
let cache = get_permission_cache();
if let Err(e) = cache.load(&pool).await {
    eprintln!("WARNING: Failed to load permission cache: {}", e);
}
```

This "graceful degradation" is actually a security vulnerability — an empty permission cache denies all non-admin access rather than failing safe.

### Migration Pattern

The original migration destroyed data before verifying the replacement:

```sql
-- BEFORE (dangerous):
TRUNCATE user_roles;
INSERT INTO user_roles (user_id, role_id)
SELECT DISTINCT user_id, role_id FROM user_department_roles
ON CONFLICT (user_id, role_id) DO NOTHING;
DROP TABLE IF EXISTS user_department_roles;
DROP TABLE IF EXISTS department_resource_access;
```

(session history) The scoped roles system that motivated this migration was already problematic — migration 016 had seeded `department_resource_access` with all departments getting all resources (via `CROSS JOIN`), making the isolation layer completely ineffective. The simplification to global roles was the correct call, but the migration pattern was unsafe.

## Solution

### Cache: Fail-Closed with `.expect()`

```rust
// AFTER (safe — fail-closed):
let cache = get_permission_cache();
cache.load(&pool).await.expect(
    "FATAL: Failed to load permission cache — refusing to start with empty permissions. \
     Check database connectivity and role_permissions table.",
);
```

The server refuses to start if the permission cache cannot be loaded. The `.expect()` message provides clear guidance for operators diagnosing the root cause.

### Migration: Preserve Data, Archive Tables

```sql
-- AFTER (safe):
INSERT INTO user_roles (user_id, role_id)
SELECT DISTINCT user_id, role_id
FROM user_department_roles
ON CONFLICT (user_id, role_id) DO NOTHING;

-- Archive instead of DROP — verify post-deploy, then clean up manually
ALTER TABLE user_department_roles RENAME TO user_department_roles_archived;
ALTER TABLE department_resource_access RENAME TO department_resource_access_archived;
```

Key changes:
- **No TRUNCATE**: Existing `user_roles` data is preserved; `ON CONFLICT DO NOTHING` handles duplicates
- **RENAME instead of DROP**: Old tables remain accessible for verification and rollback
- **Manual cleanup**: Migration comments include the DROP commands for post-verification execution

## Why This Works

**Cache fix**: The permission cache is a hard dependency for authorization. An empty cache is not "degraded service" — it's a complete authorization failure. Using `.expect()` makes this dependency explicit at the type level: the function signature doesn't change, but the runtime behavior matches the actual requirement.

**Migration fix**: The `INSERT ... ON CONFLICT DO NOTHING` pattern is idempotent — it can be re-run safely. Archiving tables via `RENAME` provides a human-verified rollback window. Together, these eliminate the two failure modes (data loss and irreversibility).

(session history) This fix addresses a pattern that recurred across the permission system's evolution: the original `check_business_permission` had an empty `dept_roles` bypass (fixed in commit `cc2f72a`) where users with no department roles could pass permission checks. The fail-open cache was a related systemic issue — the codebase had multiple places where "no data" was treated as "safe" rather than "deny".

## Prevention

- **Never use fail-open patterns for security-critical initialization** — cache, auth, key loading must all fail-closed. If the system can't verify permissions, it must refuse requests.
- **Audit all `OnceLock`/`OnceCell` singletons** — once initialized (even with empty/default data), they persist for the process lifetime. A soft failure at init time becomes permanent.
- **Never TRUNCATE before INSERT in migrations** — use `INSERT ... ON CONFLICT DO NOTHING` to preserve existing data while adding new rows.
- **Archive tables instead of DROP** — use `ALTER TABLE ... RENAME TO _archived` and document manual cleanup. This provides a natural rollback window.
- **Add permission check tests** — the review found zero tests for `check_permission_for_resource`, `check_system_permission`, and `check_business_permission`. These authorization gates need unit test coverage for all branches (super_admin, normal user, empty roles, system vs business resources).

## Related

- `docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md` — proc-macro that generates the permission check calls affected by this fix
- `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md` — migration from string literals to proto-generated enums for permission checks
- `abt/src/permission_cache.rs` — `RolePermissionCache` with `parking_lot::RwLock` and inheritance resolution
- `abt/src/lib.rs:105-111` — cache initialization site
- `abt/migrations/018_simplify_to_global_roles.sql` — the migration that was rewritten
