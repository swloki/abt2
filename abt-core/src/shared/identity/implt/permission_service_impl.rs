use std::sync::Arc;

use async_trait::async_trait;

use super::super::permission_cache::RolePermissionCache;
use super::super::permission_service::PermissionService;
use crate::shared::types::Result;

pub struct PermissionServiceImpl {
    cache: Arc<RolePermissionCache>,
}

impl PermissionServiceImpl {
    pub fn new(cache: Arc<RolePermissionCache>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl PermissionService for PermissionServiceImpl {
    async fn check_permission(
        &self,
        is_super_admin: bool,
        role_ids: &[i64],
        resource: &str,
        action: &str,
    ) -> Result<bool> {
        if is_super_admin {
            return Ok(true);
        }
        Ok(self.cache.has_permission(role_ids, resource, action).await)
    }

    async fn batch_check_permissions(
        &self,
        is_super_admin: bool,
        role_ids: &[i64],
        pairs: &[(String, String)],
    ) -> Result<Vec<bool>> {
        if is_super_admin {
            return Ok(vec![true; pairs.len()]);
        }
        let merged = self.cache.get_merged_permissions(role_ids).await;
        let results: Vec<bool> = pairs
            .iter()
            .map(|(resource, action)| {
                let key = format!("{resource}:{action}");
                merged.contains(&key)
            })
            .collect();
        Ok(results)
    }

    async fn get_user_permissions(
        &self,
        role_ids: &[i64],
    ) -> Result<Vec<String>> {
        let merged = self.cache.get_merged_permissions(role_ids).await;
        let mut perms: Vec<String> = merged.into_iter().collect();
        perms.sort();
        Ok(perms)
    }
}
