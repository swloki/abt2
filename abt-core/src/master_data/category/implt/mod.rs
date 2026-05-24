use std::sync::Arc;

use super::model::*;
use super::repo::CategoryRepo;
use super::service::CategoryService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct CategoryServiceImpl {
    repo: CategoryRepo,
    audit: Arc<dyn AuditLogService>,
}

impl CategoryServiceImpl {
    pub fn new(repo: CategoryRepo, audit: Arc<dyn AuditLogService>) -> Self {
        Self { repo, audit }
    }
}

#[async_trait::async_trait]
impl CategoryService for CategoryServiceImpl {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateCategoryReq) -> Result<i64, DomainError> {
        let meta = serde_json::to_value(CategoryMeta::default())
            .map_err(|e| DomainError::Internal(e.into()))?;

        // Insert with placeholder, get id, then fix path
        let id = self.repo.create(ctx.executor, &req.category_name, req.parent_id, "/__placeholder__/", &meta)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        let correct_path = if req.parent_id == 0 {
            format!("/{id}/")
        } else {
            let parent = self.repo.find_by_id(ctx.executor, req.parent_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("Category parent"))?;
            format!("{}{id}/", parent.path)
        };

        self.repo.update_path(ctx.executor, id, &correct_path)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "Category", id, AuditAction::Create, None, None).await?;
        Ok(id)
    }

    async fn update(&self, ctx: ServiceContext<'_>, category_id: i64, req: UpdateCategoryReq) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("Category"))?;

        self.repo.update(ctx.executor, category_id, &req)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "Category", category_id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<(), DomainError> {
        let children = self.repo.find_children_count(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        if children > 0 {
            return Err(DomainError::business_rule("分类下存在子分类，无法删除"));
        }

        let products = self.repo.find_products_count(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        if products > 0 {
            return Err(DomainError::business_rule("分类下存在关联产品，无法删除"));
        }

        self.repo.delete(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "Category", category_id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn get(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<Category, DomainError> {
        self.repo.find_by_id(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("Category"))
    }

    async fn list(&self, ctx: ServiceContext<'_>, filter: CategoryQuery, page: PageParams) -> Result<PaginatedResult<Category>, DomainError> {
        self.repo.query(ctx.executor, &filter, &page)
            .await.map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_tree(&self, ctx: ServiceContext<'_>, root_id: Option<i64>, depth_limit: Option<i32>) -> Result<Vec<CategoryTree>, DomainError> {
        let all = self.repo.find_all(ctx.executor)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        let filtered: Vec<Category> = if let Some(root) = root_id {
            all.into_iter().filter(|c| c.path.starts_with(&format!("/{root}/")) || c.category_id == root).collect()
        } else {
            all
        };

        Ok(build_tree(&filtered, 0, depth_limit.unwrap_or(i32::MAX), 0))
    }

    async fn move_to(&self, ctx: ServiceContext<'_>, category_id: i64, new_parent_id: i64) -> Result<(), DomainError> {
        let category = self.repo.find_by_id(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("Category"))?;

        if new_parent_id != 0 {
            let _parent = self.repo.find_by_id(ctx.executor, new_parent_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("Category parent"))?;
        }

        let old_prefix = category.path.clone();
        let parent_path = if new_parent_id == 0 {
            String::new()
        } else {
            self.repo.find_by_id(ctx.executor, new_parent_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?
                .map(|p| p.path).unwrap_or_default()
        };
        let new_prefix = format!("{}{category_id}/", parent_path);

        self.repo.update_parent(ctx.executor, category_id, new_parent_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.repo.update_path_subtree(ctx.executor, &old_prefix, &new_prefix)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "Category", category_id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn assign_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError> {
        self.repo.assign_products(ctx.executor, category_id, &product_ids)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        let count = self.repo.find_products_count(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        self.repo.update_meta_count(ctx.executor, category_id, count)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn remove_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError> {
        self.repo.remove_products(ctx.executor, category_id, &product_ids)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        let count = self.repo.find_products_count(ctx.executor, category_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        self.repo.update_meta_count(ctx.executor, category_id, count)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }
}

fn build_tree(categories: &[Category], parent_id: i64, depth_limit: i32, current_depth: i32) -> Vec<CategoryTree> {
    if current_depth >= depth_limit {
        return vec![];
    }
    categories
        .iter()
        .filter(|c| c.parent_id == parent_id)
        .map(|c| {
            let meta: CategoryMeta = serde_json::from_value(c.meta.clone()).unwrap_or_default();
            CategoryTree {
                category_id: c.category_id,
                category_name: c.category_name.clone(),
                parent_id: c.parent_id,
                path: c.path.clone(),
                children: build_tree(categories, c.category_id, depth_limit, current_depth + 1),
                meta,
            }
        })
        .collect()
}
