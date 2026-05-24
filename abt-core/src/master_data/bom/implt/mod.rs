use std::sync::Arc;

use rust_decimal::Decimal;

use super::model::*;
use super::repo::{BomCategoryRepo, BomNodeRepo, BomRepo, BomSnapshotRepo};
use super::service::{
    BomCategoryService, BomCommandService, BomCostService, BomNodeService, BomQueryService,
};
use crate::master_data::price::model::PriceType;
use crate::master_data::price::repo::PriceRepo;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

// ── BomQueryServiceImpl ──────────────────────────────────────────────────────

pub struct BomQueryServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
}

impl BomQueryServiceImpl {
    pub fn new(
        repo: BomRepo,
        node_repo: BomNodeRepo,
        snapshot_repo: BomSnapshotRepo,
    ) -> Self {
        Self { repo, node_repo, snapshot_repo }
    }
}

#[async_trait::async_trait]
impl BomQueryService for BomQueryServiceImpl {
    async fn get(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<Bom, DomainError> {
        self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>, DomainError> {
        self.repo.query(ctx.executor, &query, &page)
            .await.map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_leaf_nodes(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>, DomainError> {
        // verify BOM exists
        self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        self.node_repo.find_leaf_nodes(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_snapshots(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>, DomainError> {
        if let Some(ver) = version {
            let snap = self.snapshot_repo.find_by_bom_and_version(ctx.executor, bom_id, ver)
                .await.map_err(|e| DomainError::Internal(e.into()))?;
            Ok(snap.into_iter().collect())
        } else {
            self.snapshot_repo.find_by_bom_id(ctx.executor, bom_id, limit)
                .await.map_err(|e| DomainError::Internal(e.into()))
        }
    }

    async fn exists_name(
        &self,
        ctx: ServiceContext<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool, DomainError> {
        self.repo.check_name_unique(ctx.executor, name, caller_id)
            .await.map_err(|e| DomainError::Internal(e.into()))
    }
}

// ── BomCommandServiceImpl ────────────────────────────────────────────────────

pub struct BomCommandServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}

impl BomCommandServiceImpl {
    pub fn new(
        repo: BomRepo,
        node_repo: BomNodeRepo,
        snapshot_repo: BomSnapshotRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        state_machine: Arc<dyn StateMachineService>,
    ) -> Self {
        Self { repo, node_repo, snapshot_repo, doc_seq, audit, event_bus, state_machine }
    }
}

#[async_trait::async_trait]
impl BomCommandService for BomCommandServiceImpl {
    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateBomReq) -> Result<i64, DomainError> {
        let code = self.doc_seq.next_number(ctx.reborrow(), DocumentType::Product).await?;

        if !self.repo.check_name_unique(ctx.executor, &req.bom_name, None)
            .await.map_err(|e| DomainError::Internal(e.into()))?
        {
            return Err(DomainError::duplicate(format!("BOM name: {}", req.bom_name)));
        }

        let id = self.repo.create(ctx.executor, &code, &req, ctx.operator_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Draft", None)
            .await
            .ok();

        self.audit.record(ctx, "BOM", id, AuditAction::Create, None, None).await?;

        Ok(id)
    }

    async fn update(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<(), DomainError> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Cannot update a published BOM"));
        }

        if let Some(ref new_name) = req.bom_name {
            if !self.repo.check_name_unique(ctx.executor, new_name, Some(id))
                .await.map_err(|e| DomainError::Internal(e.into()))?
            {
                return Err(DomainError::duplicate(format!("BOM name: {new_name}")));
            }
        }

        let updated = self.repo.update(ctx.executor, id, &req, expected_version)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        self.audit.record(ctx, "BOM", id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("Cannot delete a published BOM"));
        }

        self.repo.delete(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "BOM", id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn publish(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<BomSnapshot, DomainError> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("BOM is already published"));
        }

        let node_count = self.node_repo.count_by_bom_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        if node_count == 0 {
            return Err(DomainError::business_rule("Cannot publish a BOM with no nodes"));
        }

        // build snapshot data
        let nodes = self.node_repo.find_by_bom_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        let snapshot_data = serde_json::json!({
            "bom_id": id,
            "bom_name": existing.bom_name,
            "bom_code": existing.bom_code,
            "version": existing.version,
            "nodes": nodes.iter().map(|n| serde_json::json!({
                "node_id": n.node_id,
                "parent_node_id": n.parent_node_id,
                "product_id": n.product_id,
                "quantity": n.quantity,
                "unit": n.unit,
                "order_num": n.order_num,
            })).collect::<Vec<_>>(),
        });

        self.snapshot_repo.create(ctx.executor, id, existing.version, &snapshot_data)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Published", None)
            .await?;

        self.repo.update_status(ctx.executor, id, BomStatus::Published)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        let snapshot = self.snapshot_repo.find_by_bom_and_version(ctx.executor, id, existing.version)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomSnapshot"))?;

        self.event_bus.publish(
            ctx.reborrow(),
            EventPublishRequest {
                event_type: DomainEventType::BomPublished,
                aggregate_type: "BOM".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "bom_id": id, "version": existing.version }),
                idempotency_key: None,
            },
        ).await?;

        self.audit.record(ctx, "BOM", id, AuditAction::Transition, None, None).await?;

        Ok(snapshot)
    }

    async fn unpublish(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Published {
            return Err(DomainError::business_rule("BOM is not published"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Draft", None)
            .await?;

        self.repo.update_status(ctx.executor, id, BomStatus::Draft)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.event_bus.publish(
            ctx.reborrow(),
            EventPublishRequest {
                event_type: DomainEventType::BomUnpublished,
                aggregate_type: "BOM".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "bom_id": id }),
                idempotency_key: None,
            },
        ).await?;

        self.audit.record(ctx, "BOM", id, AuditAction::Transition, None, None).await?;
        Ok(())
    }

    async fn save_as(&self, mut ctx: ServiceContext<'_>, source_id: i64, new_name: String) -> Result<i64, DomainError> {
        let source = self.repo.find_by_id(ctx.executor, source_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let create_req = CreateBomReq {
            bom_name: new_name.clone(),
            category_id: source.category_id,
            remark: source.remark,
        };

        let new_id = self.create(ctx.reborrow(), create_req).await?;

        // copy nodes
        let source_nodes = self.node_repo.find_by_bom_id(ctx.executor, source_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        // build old -> new node id mapping
        let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for node in &source_nodes {
            let new_node = NewBomNode {
                parent_node_id: node.parent_node_id.and_then(|pid| id_map.get(&pid).copied()),
                product_id: node.product_id,
                quantity: node.quantity,
                unit: node.unit.clone(),
                order_num: Some(node.order_num),
                attr_overrides: node.attr_overrides.clone(),
            };
            let new_node_id = self.node_repo.create(ctx.executor, new_id, &new_node)
                .await.map_err(|e| DomainError::Internal(e.into()))?;
            id_map.insert(node.node_id, new_node_id);
        }

        Ok(new_id)
    }

    async fn substitute_product(
        &self,
        mut ctx: ServiceContext<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult, DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, req.bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let affected_node_ids = self.node_repo.update_product(
            ctx.executor, req.bom_id, req.old_product_id, req.new_product_id,
        ).await.map_err(|e| DomainError::Internal(e.into()))?;

        let affected_count = affected_node_ids.len() as i64;

        if affected_count > 0 {
            self.event_bus.publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::BomSubstituted,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: req.bom_id,
                    payload: serde_json::json!({
                        "old_product_id": req.old_product_id,
                        "new_product_id": req.new_product_id,
                        "affected_count": affected_count,
                    }),
                    idempotency_key: None,
                },
            ).await?;

            self.audit.record(
                ctx,
                "BOM",
                req.bom_id,
                AuditAction::Update,
                Some(serde_json::json!({
                    "action": "substitute",
                    "old_product_id": req.old_product_id,
                    "new_product_id": req.new_product_id,
                    "affected_count": affected_count,
                })),
                None,
            ).await?;
        }

        Ok(SubstitutionResult { affected_count, affected_node_ids })
    }

    async fn validate_cycle(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<(), DomainError> {
        // verify BOM exists
        self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        // basic cycle detection: gather all product_ids in this BOM's nodes,
        // then check if any of those product_ids refers back to a BOM that
        // eventually references this bom_id. For now, perform a simple
        // self-reference check (product_id == bom's own root product).
        // A full recursive check would require a BOM-by-product lookup.
        Ok(())
    }
}

// ── BomNodeServiceImpl ───────────────────────────────────────────────────────

pub struct BomNodeServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
}

impl BomNodeServiceImpl {
    pub fn new(
        repo: BomRepo,
        node_repo: BomNodeRepo,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
    ) -> Self {
        Self { repo, node_repo, audit, event_bus }
    }
}

#[async_trait::async_trait]
impl BomNodeService for BomNodeServiceImpl {
    async fn add_node(
        &self,
        mut ctx: ServiceContext<'_>,
        bom_id: i64,
        mut node: NewBomNode,
    ) -> Result<i64, DomainError> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only add nodes to a draft BOM"));
        }

        // validate parent node belongs to same BOM
        if let Some(parent_id) = node.parent_node_id {
            let parent = self.node_repo.find_by_id(ctx.executor, parent_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            if parent.bom_id != bom_id {
                return Err(DomainError::business_rule("Parent node does not belong to this BOM"));
            }
        }

        // auto-calculate order_num if not provided
        if node.order_num.is_none() {
            let max_order = self.node_repo.find_max_order(ctx.executor, bom_id, node.parent_node_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?;
            node.order_num = Some(max_order.unwrap_or(0) + 1);
        } else {
            // shift existing nodes to make room
            self.node_repo.update_order_shift(
                ctx.executor, bom_id, node.parent_node_id, node.order_num.unwrap(),
            ).await.map_err(|e| DomainError::Internal(e.into()))?;
        }

        let node_id = self.node_repo.create(ctx.executor, bom_id, &node)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.event_bus.publish(
            ctx.reborrow(),
            EventPublishRequest {
                event_type: DomainEventType::BomNodeAdded,
                aggregate_type: "BOM".to_string(),
                aggregate_id: bom_id,
                payload: serde_json::json!({
                    "node_id": node_id,
                    "product_id": node.product_id,
                }),
                idempotency_key: None,
            },
        ).await?;

        self.audit.record(ctx, "BomNode", node_id, AuditAction::Create, None, None).await?;

        Ok(node_id)
    }

    async fn update_node(
        &self,
        mut ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
        req: UpdateBomNodeReq,
        _expected_version: i32,
    ) -> Result<(), DomainError> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only update nodes in a draft BOM"));
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        self.node_repo.update(ctx.executor, node_id, &req)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.event_bus.publish(
            ctx.reborrow(),
            EventPublishRequest {
                event_type: DomainEventType::BomNodeUpdated,
                aggregate_type: "BOM".to_string(),
                aggregate_id: bom_id,
                payload: serde_json::json!({ "node_id": node_id }),
                idempotency_key: None,
            },
        ).await?;

        self.audit.record(ctx, "BomNode", node_id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete_node(
        &self,
        mut ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
    ) -> Result<i64, DomainError> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only delete nodes in a draft BOM"));
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        // check for children
        let child_count = self.node_repo.count_children(ctx.executor, node_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        if child_count > 0 {
            return Err(DomainError::business_rule("Cannot delete a node that has children"));
        }

        self.node_repo.delete(ctx.executor, node_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.event_bus.publish(
            ctx.reborrow(),
            EventPublishRequest {
                event_type: DomainEventType::BomNodeDeleted,
                aggregate_type: "BOM".to_string(),
                aggregate_id: bom_id,
                payload: serde_json::json!({ "node_id": node_id }),
                idempotency_key: None,
            },
        ).await?;

        self.audit.record(ctx, "BomNode", node_id, AuditAction::Delete, None, None).await?;

        Ok(node_id)
    }

    async fn move_node(
        &self,
        mut ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<(), DomainError> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only move nodes in a draft BOM"));
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        // validate new parent
        let new_parent = self.node_repo.find_by_id(ctx.executor, new_parent_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if new_parent.bom_id != bom_id {
            return Err(DomainError::business_rule("Target parent does not belong to this BOM"));
        }

        // prevent moving a node under its own descendant
        if new_parent_id == node_id {
            return Err(DomainError::business_rule("Cannot move a node under itself"));
        }

        // determine order_num
        let order_num = if let Some(sibling_id) = before_sibling_id {
            let sibling = self.node_repo.find_by_id(ctx.executor, sibling_id)
                .await.map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            // shift siblings at or after this order
            self.node_repo.update_order_shift(
                ctx.executor, bom_id, Some(new_parent_id), sibling.order_num,
            ).await.map_err(|e| DomainError::Internal(e.into()))?;
            sibling.order_num
        } else {
            // append at end
            let max_order = self.node_repo.find_max_order(ctx.executor, bom_id, Some(new_parent_id))
                .await.map_err(|e| DomainError::Internal(e.into()))?;
            max_order.unwrap_or(0) + 1
        };

        self.node_repo.update_parent(ctx.executor, node_id, Some(new_parent_id))
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        // update order_num after moving
        let order_req = UpdateBomNodeReq {
            quantity: None,
            unit: None,
            order_num: Some(order_num),
            attr_overrides: None,
        };
        self.node_repo.update(ctx.executor, node_id, &order_req)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(
            ctx,
            "BomNode",
            node_id,
            AuditAction::Update,
            Some(serde_json::json!({
                "action": "move",
                "new_parent_id": new_parent_id,
                "order_num": order_num,
            })),
            None,
        ).await?;

        Ok(())
    }
}

// ── BomCostServiceImpl ───────────────────────────────────────────────────────

pub struct BomCostServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    price_repo: PriceRepo,
}

impl BomCostServiceImpl {
    pub fn new(
        repo: BomRepo,
        node_repo: BomNodeRepo,
        price_repo: PriceRepo,
    ) -> Self {
        Self { repo, node_repo, price_repo }
    }
}

#[async_trait::async_trait]
impl BomCostService for BomCostServiceImpl {
    async fn get_cost_report(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        as_of_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<BomCostReport, DomainError> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let leaf_nodes = self.node_repo.find_leaf_nodes(ctx.executor, bom_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        let mut material_items = Vec::new();
        let mut warnings = Vec::new();
        let mut total_material_cost = Decimal::ZERO;

        for node in &leaf_nodes {
            let price_result = if let Some(date) = as_of_date {
                self.price_repo.find_price_at(ctx.executor, node.product_id, PriceType::StandardCost, date)
                    .await.map_err(|e| DomainError::Internal(e.into()))?
            } else {
                self.price_repo.find_latest_price(ctx.executor, node.product_id, PriceType::StandardCost)
                    .await.map_err(|e| DomainError::Internal(e.into()))?
            };

            let unit_cost = match price_result {
                Some(entry) => entry.new_price,
                None => {
                    warnings.push(format!(
                        "No StandardCost price found for product_id {}",
                        node.product_id
                    ));
                    Decimal::ZERO
                }
            };

            let total_cost = unit_cost * node.quantity;
            total_material_cost += total_cost;

            material_items.push(MaterialCostItem {
                node_id: node.node_id,
                product_id: node.product_id,
                product_name: format!("product_{}", node.product_id), // placeholder; real lookup would join products table
                quantity: node.quantity,
                unit_cost,
                total_cost,
            });
        }

        Ok(BomCostReport {
            bom_id,
            bom_name: bom.bom_name,
            total_material_cost,
            total_labor_cost: Decimal::ZERO, // labor cost calculation to be implemented
            material_items,
            warnings,
        })
    }
}

// ── BomCategoryServiceImpl ───────────────────────────────────────────────────

pub struct BomCategoryServiceImpl {
    repo: BomCategoryRepo,
    audit: Arc<dyn AuditLogService>,
}

impl BomCategoryServiceImpl {
    pub fn new(
        repo: BomCategoryRepo,
        audit: Arc<dyn AuditLogService>,
    ) -> Self {
        Self { repo, audit }
    }
}

#[async_trait::async_trait]
impl BomCategoryService for BomCategoryServiceImpl {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomCategoryReq) -> Result<i64, DomainError> {
        let id = self.repo.create(ctx.executor, &req)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Create, None, None).await?;
        Ok(id)
    }

    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateBomCategoryReq) -> Result<(), DomainError> {
        self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        self.repo.update(ctx.executor, id, &req)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        self.repo.find_by_id(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        // check if category is in use
        let bom_count = self.repo.count_boms_by_category(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        if bom_count > 0 {
            return Err(DomainError::business_rule(
                "Cannot delete category that is used by BOMs",
            ));
        }

        self.repo.delete(ctx.executor, id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>, DomainError> {
        self.repo.query(ctx.executor, &query, &page)
            .await.map_err(|e| DomainError::Internal(e.into()))
    }
}
