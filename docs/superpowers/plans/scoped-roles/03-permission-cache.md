# Task 03: Permission Cache with Role Inheritance

**Goal:** Create an in-memory permission cache that loads all role permissions at startup, resolves inheritance chains (with cycle detection), and provides fast lookups.

**Depends on:** Task 01 (migration — parent_role_id column)

**Files:**
- Create: `abt/src/permission_cache.rs`
- Modify: `abt/src/lib.rs` — register module and init on startup

## Steps

- [ ] **Step 1: Create the permission cache module**

Create `abt/src/permission_cache.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::PgPool;
use anyhow::Result;

/// In-memory cache of role permissions with inheritance resolution.
///
/// Loaded at startup from role_permissions + roles (parent_role_id).
/// When role/permission data changes, call `refresh()` to reload.
pub struct RolePermissionCache {
    /// role_id -> set of fully-resolved permission codes ("resource:action")
    cache: RwLock<HashMap<i64, HashSet<String>>>,
}

impl RolePermissionCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Load permissions from database, resolve inheritance, detect cycles.
    /// Call at startup and after role/permission changes.
    pub async fn load(&self, pool: &PgPool) -> Result<()> {
        // 1. Load all roles with parent_role_id
        let roles: Vec<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT role_id, parent_role_id FROM roles"
        )
        .fetch_all(pool)
        .await?;

        // 2. Load all direct role_permissions
        let perms: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT role_id, CONCAT(resource_code, ':', action_code) as "perm"
            FROM role_permissions
            "#
        )
        .fetch_all(pool)
        .await?;

        // 3. Build direct permissions map: role_id -> set of permission codes
        let mut direct: HashMap<i64, HashSet<String>> = HashMap::new();
        for (role_id, perm) in perms {
            direct.entry(role_id).or_default().insert(perm);
        }

        // 4. Build parent map
        let parent_map: HashMap<i64, Option<i64>> = roles
            .into_iter()
            .map(|(id, parent)| (id, parent))
            .collect();

        // 5. Detect cycles via DFS
        Self::detect_cycles(&parent_map)?;

        // 6. Resolve inheritance for each role
        let mut resolved: HashMap<i64, HashSet<String>> = HashMap::new();
        for &role_id in parent_map.keys() {
            let permissions = Self::resolve_permissions(role_id, &parent_map, &direct, &mut resolved);
            resolved.insert(role_id, permissions);
        }

        // 7. Swap cache
        *self.cache.write().await = resolved;

        Ok(())
    }

    /// Get the resolved permissions for a role (includes inherited).
    pub async fn get_permissions(&self, role_id: i64) -> HashSet<String> {
        self.cache
            .read()
            .await
            .get(&role_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get merged permissions for multiple roles (union).
    pub async fn get_merged_permissions(&self, role_ids: &[i64]) -> HashSet<String> {
        let cache = self.cache.read().await;
        let mut merged = HashSet::new();
        for &role_id in role_ids {
            if let Some(perms) = cache.get(&role_id) {
                merged.extend(perms.iter().cloned());
            }
        }
        merged
    }

    /// Check if any of the given roles has a specific permission.
    pub async fn has_permission(&self, role_ids: &[i64], resource: &str, action: &str) -> bool {
        let required = format!("{}:{}", resource, action);
        let cache = self.cache.read().await;
        for &role_id in role_ids {
            if let Some(perms) = cache.get(&role_id) {
                if perms.contains(&required) {
                    return true;
                }
            }
        }
        false
    }

    /// Recursively resolve permissions along the inheritance chain.
    fn resolve_permissions(
        role_id: i64,
        parent_map: &HashMap<i64, Option<i64>>,
        direct: &HashMap<i64, HashSet<String>>,
        resolved: &mut HashMap<i64, HashSet<String>>,
    ) -> HashSet<String> {
        if let Some(perms) = resolved.get(&role_id) {
            return perms.clone();
        }

        let mut permissions = direct
            .get(&role_id)
            .cloned()
            .unwrap_or_default();

        if let Some(Some(parent_id)) = parent_map.get(&role_id) {
            let parent_perms = Self::resolve_permissions(*parent_id, parent_map, direct, resolved);
            permissions.extend(parent_perms);
        }

        permissions
    }

    /// Detect cycles in the inheritance chain using DFS.
    fn detect_cycles(parent_map: &HashMap<i64, Option<i64>>) -> Result<()> {
        let mut visited: HashSet<i64> = HashSet::new();
        let mut in_stack: HashSet<i64> = HashSet::new();

        for &role_id in parent_map.keys() {
            Self::dfs_cycle(role_id, parent_map, &mut visited, &mut in_stack)?;
        }
        Ok(())
    }

    fn dfs_cycle(
        role_id: i64,
        parent_map: &HashMap<i64, Option<i64>>,
        visited: &mut HashSet<i64>,
        in_stack: &mut HashSet<i64>,
    ) -> Result<()> {
        if in_stack.contains(&role_id) {
            anyhow::bail!(
                "Circular role inheritance detected involving role_id {}",
                role_id
            );
        }
        if visited.contains(&role_id) {
            return Ok(());
        }

        visited.insert(role_id);
        in_stack.insert(role_id);

        if let Some(Some(parent_id)) = parent_map.get(&role_id) {
            Self::dfs_cycle(*parent_id, parent_map, visited, in_stack)?;
        }

        in_stack.remove(&role_id);
        Ok(())
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Modify `abt/src/lib.rs` — add near other module declarations:

```rust
mod permission_cache;
pub use permission_cache::RolePermissionCache;
```

- [ ] **Step 3: Add global cache instance and init**

Modify `abt/src/lib.rs` — add a global `OnceLock` for the cache:

```rust
static PERMISSION_CACHE: OnceLock<RolePermissionCache> = OnceLock::new();

/// Get the global permission cache
pub fn get_permission_cache() -> &'static RolePermissionCache {
    PERMISSION_CACHE.get_or_init(RolePermissionCache::new)
}
```

- [ ] **Step 4: Load cache during context init**

Modify `init_context_with_pool` in `abt/src/lib.rs` — add cache loading after context is set:

```rust
pub async fn init_context_with_pool(pool: PgPool) {
    // ... existing double-check locking code ...
    let ctx = AppContext { pool };
    CONTEXT.set(ctx).ok();

    // Load permission cache
    let cache = get_permission_cache();
    if let Err(e) = cache.load(&pool).await {
        eprintln!("WARNING: Failed to load permission cache: {}", e);
    }
}
```

- [ ] **Step 5: Build to verify**

Run: `cd e:/work/abt && cargo build -p abt`
Expected: Compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add abt/src/permission_cache.rs abt/src/lib.rs
git commit -m "feat: add RolePermissionCache with inheritance resolution and cycle detection"
```
