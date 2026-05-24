use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use sqlx::postgres::PgPool;
use tokio::sync::RwLock;

use super::repo::IdentityRepo;

const CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Cached permissions with a loaded_at timestamp for TTL-based staleness check
struct CacheState {
    permissions: HashMap<i64, HashSet<String>>,
    loaded_at: Instant,
}

impl CacheState {
    fn new(permissions: HashMap<i64, HashSet<String>>) -> Self {
        Self {
            permissions,
            loaded_at: Instant::now(),
        }
    }

    fn is_stale(&self) -> bool {
        self.loaded_at.elapsed().as_secs() > CACHE_TTL_SECS
    }
}

/// In-memory permission cache: role_id → resolved (inherited) permissions
pub struct RolePermissionCache {
    state: RwLock<CacheState>,
    pool: Arc<PgPool>,
}

impl RolePermissionCache {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            state: RwLock::new(CacheState::new(HashMap::new())),
            pool,
        }
    }

    /// Full load: fetch all role_permissions + parent_role hierarchy, then DFS resolve inheritance.
    /// Circular parent references cause a hard error.
    pub async fn load(&self, pool: &PgPool) -> Result<(), crate::shared::types::error::DomainError> {
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| crate::shared::types::error::DomainError::Internal(e.into()))?;

        // 1. Load all direct permissions: role_id → (resource_code, action)
        let perms = IdentityRepo::get_all_role_permissions(&mut conn)
            .await
            .map_err(|e| crate::shared::types::error::DomainError::Internal(e.into()))?;

        // 2. Load parent mapping: role_id → parent_role_id
        let parent_map_rows = IdentityRepo::get_role_parent_map(&mut conn)
            .await
            .map_err(|e| crate::shared::types::error::DomainError::Internal(e.into()))?;

        let parent_map: HashMap<i64, Option<i64>> = parent_map_rows.into_iter().collect();

        // 3. Build direct permissions map
        let mut direct: HashMap<i64, HashSet<String>> = HashMap::new();
        for (role_id, resource, action) in &perms {
            let key = format!("{resource}:{action}");
            direct.entry(*role_id).or_default().insert(key);
        }

        // 4. DFS resolve with cycle detection
        let mut resolved: HashMap<i64, HashSet<String>> = HashMap::new();
        let mut visiting: HashSet<i64> = HashSet::new();
        let mut visited: HashSet<i64> = HashSet::new();

        for &role_id in parent_map.keys() {
            Self::resolve_dfs(
                role_id,
                &parent_map,
                &direct,
                &mut resolved,
                &mut visiting,
                &mut visited,
            )?;
        }

        // 5. Write cache
        let count = resolved.len();
        *self.state.write().await = CacheState::new(resolved);
        tracing::info!(count, "RolePermissionCache loaded");
        Ok(())
    }

    /// Auto-reload if cache is stale (TTL-based). Transparent to callers.
    async fn ensure_fresh(&self) -> Result<(), crate::shared::types::error::DomainError> {
        if self.state.read().await.is_stale() {
            self.load(&self.pool).await?;
        }
        Ok(())
    }

    /// Merge permissions across multiple roles
    pub async fn get_merged_permissions(&self, role_ids: &[i64]) -> HashSet<String> {
        let _ = self.ensure_fresh().await;
        let state = self.state.read().await;
        let mut merged = HashSet::new();
        for &role_id in role_ids {
            if let Some(perms) = state.permissions.get(&role_id) {
                merged.extend(perms.iter().cloned());
            }
        }
        merged
    }

    /// Check if any of the given roles has the permission
    pub async fn has_permission(&self, role_ids: &[i64], resource: &str, action: &str) -> bool {
        let merged = self.get_merged_permissions(role_ids).await;
        let key = format!("{resource}:{action}");
        merged.contains(&key)
    }

    /// Reload cache from database
    pub async fn reload(&self, pool: &PgPool) -> Result<(), crate::shared::types::error::DomainError> {
        self.load(pool).await
    }

    // -----------------------------------------------------------------------
    // DFS resolution with cycle detection
    // -----------------------------------------------------------------------

    fn resolve_dfs(
        role_id: i64,
        parent_map: &HashMap<i64, Option<i64>>,
        direct: &HashMap<i64, HashSet<String>>,
        resolved: &mut HashMap<i64, HashSet<String>>,
        visiting: &mut HashSet<i64>,
        visited: &mut HashSet<i64>,
    ) -> Result<(), crate::shared::types::error::DomainError> {
        if visited.contains(&role_id) {
            return Ok(());
        }
        if visiting.contains(&role_id) {
            return Err(crate::shared::types::error::DomainError::BusinessRule(
                format!("Circular role hierarchy detected at role_id={role_id}"),
            ));
        }

        visiting.insert(role_id);

        // Start with own direct permissions
        let mut perms: HashSet<String> = direct.get(&role_id).cloned().unwrap_or_default();

        // Inherit from parent
        if let Some(Some(parent_id)) = parent_map.get(&role_id) {
            // Ensure parent is resolved first
            Self::resolve_dfs(*parent_id, parent_map, direct, resolved, visiting, visited)?;
            if let Some(parent_perms) = resolved.get(parent_id) {
                perms.extend(parent_perms.iter().cloned());
            }
        }

        visiting.remove(&role_id);
        visited.insert(role_id);
        resolved.insert(role_id, perms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cache() -> RolePermissionCache {
        // Tests that don't need DB can pass a dummy pool (tests won't trigger reload with fresh cache)
        let pool = Arc::new(PgPool::connect_lazy("postgres://__test__").expect("lazy connect"));
        RolePermissionCache::new(pool)
    }

    #[tokio::test]
    async fn test_empty_cache() {
        let cache = make_test_cache();
        let perms = cache.get_merged_permissions(&[]).await;
        assert!(perms.is_empty());
    }

    #[tokio::test]
    async fn test_has_permission_empty() {
        let cache = make_test_cache();
        assert!(!cache.has_permission(&[1], "PRODUCT", "read").await);
    }

    #[tokio::test]
    async fn test_manual_cache_population() {
        let cache = make_test_cache();
        // Manually write to cache
        let mut guard = cache.state.write().await;
        let mut set = HashSet::new();
        set.insert("PRODUCT:read".to_string());
        guard.permissions.insert(1, set);
        drop(guard);

        assert!(cache.has_permission(&[1], "PRODUCT", "read").await);
        assert!(!cache.has_permission(&[1], "PRODUCT", "delete").await);
    }

    #[tokio::test]
    async fn test_merged_permissions_multiple_roles() {
        let cache = make_test_cache();
        let mut guard = cache.state.write().await;
        let mut set1 = HashSet::new();
        set1.insert("PRODUCT:read".to_string());
        guard.permissions.insert(1, set1);

        let mut set2 = HashSet::new();
        set2.insert("PRODUCT:delete".to_string());
        guard.permissions.insert(2, set2);
        drop(guard);

        let merged = cache.get_merged_permissions(&[1, 2]).await;
        assert!(merged.contains("PRODUCT:read"));
        assert!(merged.contains("PRODUCT:delete"));
    }
}
