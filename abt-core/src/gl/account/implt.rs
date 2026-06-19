use sqlx::PgPool;
use rust_decimal::Decimal;

use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result};

use super::model::*;
use super::repo::GlAccountRepo;
use super::service::{GlAccountService, GlAccountNode};

pub struct GlAccountServiceImpl {
    pool: PgPool,
}

impl GlAccountServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GlAccountService for GlAccountServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateGlAccountReq,
    ) -> Result<i64> {
        // Validation
        if req.code.trim().is_empty() || req.name.trim().is_empty() {
            return Err(DomainError::validation("code and name are required"));
        }

        // Check code uniqueness
        if GlAccountRepo::get_by_code(db, &req.code).await?.is_some() {
            return Err(DomainError::duplicate("GlAccount.code"));
        }

        // Validate parent exists if provided
        if let Some(parent_id) = req.parent_id {
            if GlAccountRepo::get_by_id(db, parent_id).await?.is_none() {
                return Err(DomainError::not_found("GlAccount parent"));
            }
        }

        // Validate opening_balance and currency
        if req.opening_balance < Decimal::ZERO {
            return Err(DomainError::validation("opening_balance must be non-negative"));
        }

        if req.currency.trim().is_empty() {
            return Err(DomainError::validation("currency is required"));
        }

        // Create account
        let id = GlAccountRepo::create(db, &req).await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "GlAccount",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateGlAccountReq,
    ) -> Result<()> {
        // Fetch current
        let _account = GlAccountRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("GlAccount"))?;

        // Update with optimistic lock
        let rows = GlAccountRepo::update(db, id, &req).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Audit log
        let changes = serde_json::json!({
            "name": req.name,
            "disabled": req.disabled,
        });
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "GlAccount",
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: Some(changes),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<GlAccount> {
        GlAccountRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("GlAccount"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: GlAccountFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<GlAccount>> {
        let (items, total) = GlAccountRepo::query(
            db,
            &filter,
            &page,
            ctx.data_scope,
            ctx.operator_id,
            ctx.department_id,
        )
        .await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn get_tree(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<GlAccountNode>> {
        // Fetch all accounts (no pagination)
        let page = PageParams::new(1, 10000); // Large page size to get all
        let filter = GlAccountFilter::default();
        let (accounts, _) = GlAccountRepo::query(
            db,
            &filter,
            &page,
            ctx.data_scope,
            ctx.operator_id,
            ctx.department_id,
        )
        .await?;

        // Build tree in-memory
        let tree = build_tree(accounts);
        Ok(tree)
    }
}

/// Build tree structure from flat account list
fn build_tree(accounts: Vec<GlAccount>) -> Vec<GlAccountNode> {
    use std::collections::HashMap;

    let mut node_map: HashMap<i64, GlAccountNode> = HashMap::new();
    let mut root_ids: Vec<i64> = Vec::new();

    // First pass: create all nodes
    for account in accounts {
        let id = account.id;
        let node = GlAccountNode {
            account,
            children: Vec::new(),
        };
        node_map.insert(id, node);
    }

    // Collect all IDs before we start moving nodes
    let all_ids: Vec<i64> = node_map.keys().copied().collect();

    // Second pass: build hierarchy
    for &id in &all_ids {
        if let Some(node) = node_map.remove(&id) {
            if let Some(parent_id) = node.account.parent_id {
                // Try to add to parent's children
                if node_map.get_mut(&parent_id).is_some() {
                    // Parent exists - add as child
                    if let Some(parent) = node_map.get_mut(&parent_id) {
                        parent.children.push(node);
                    }
                } else {
                    // Parent doesn't exist - treat as root
                    root_ids.push(id);
                    node_map.insert(id, node);
                }
            } else {
                // No parent - it's a root
                root_ids.push(id);
                node_map.insert(id, node);
            }
        }
    }

    // Build final roots vector
    let mut roots = Vec::new();
    for id in root_ids {
        if let Some(node) = node_map.remove(&id) {
            roots.push(node);
        }
    }

    // Sort roots by code
    roots.sort_by(|a, b| a.account.code.cmp(&b.account.code));

    // Recursively sort children
    for node in &mut roots {
        sort_children(node);
    }

    roots
}

/// Recursively sort children by code
fn sort_children(node: &mut GlAccountNode) {
    node.children.sort_by(|a, b| a.account.code.cmp(&b.account.code));
    for child in &mut node.children {
        sort_children(child);
    }
}
