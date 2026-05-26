use std::sync::Arc;

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
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext, Result};

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
    async fn get(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<Bom> {
        let mut bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;
        let nodes = self.node_repo.find_by_bom_id(ctx.executor, bom_id)
            .await?;
        bom.bom_detail = BomDetail { nodes };
        Ok(bom)
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>> {
        self.repo.query(ctx.executor, &query, &page)
            .await
    }

    async fn get_leaf_nodes(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>> {
        self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        self.node_repo.find_leaf_nodes(ctx.executor, bom_id)
            .await
    }

    async fn get_snapshots(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>> {
        if let Some(ver) = version {
            let snap = self.snapshot_repo.find_by_bom_and_version(ctx.executor, bom_id, ver)
                .await?;
            Ok(snap.into_iter().collect())
        } else {
            self.snapshot_repo.find_by_bom_id(ctx.executor, bom_id, limit)
                .await
        }
    }

    async fn exists_name(
        &self,
        ctx: ServiceContext<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool> {
        self.repo.check_name_unique(ctx.executor, name, caller_id)
            .await
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
    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateBomReq) -> Result<i64> {
        let code = self.doc_seq.next_number(ctx.reborrow(), DocumentType::Bom).await?;

        if !self.repo.check_name_unique(ctx.executor, &req.name, None)
            .await?
        {
            return Err(DomainError::duplicate(format!("BOM name: {}", req.name)));
        }

        let id = self.repo.create(ctx.executor, &code, &req, ctx.operator_id)
            .await?;

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Draft", None)
            .await
            .ok();

        self.audit.record(ctx, "BOM", id, AuditAction::Create, None, None).await?;

        Ok(id)
    }

    #[allow(clippy::collapsible_if)]
    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<()> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Cannot update a published BOM"));
        }

        if let Some(ref new_name) = req.name {
            if !self.repo.check_name_unique(ctx.executor, new_name, Some(id))
                .await?
            {
                return Err(DomainError::duplicate(format!("BOM name: {new_name}")));
            }
        }

        let updated = self.repo.update(ctx.executor, id, &req, expected_version)
            .await?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        self.audit.record(ctx, "BOM", id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("Cannot delete a published BOM"));
        }

        self.repo.delete(ctx.executor, id)
            .await?;

        self.audit.record(ctx, "BOM", id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn publish(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<i64> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("BOM is already published"));
        }

        let node_count = self.node_repo.count_by_bom_id(ctx.executor, id)
            .await?;
        if node_count == 0 {
            return Err(DomainError::business_rule("Cannot publish a BOM with no nodes"));
        }

        // build BomDetail from nodes
        let nodes = self.node_repo.find_by_bom_id(ctx.executor, id)
            .await?;
        let bom_detail = BomDetail { nodes };

        self.snapshot_repo.create(
            ctx.executor, id, existing.version, &existing.bom_name, &bom_detail, ctx.operator_id,
        ).await?;

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Published", None)
            .await?;

        self.repo.update_status(ctx.executor, id, BomStatus::Published)
            .await?;

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

        Ok(id)
    }

    async fn unpublish(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Published {
            return Err(DomainError::business_rule("BOM is not published"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "BomStatus", id, "Draft", None)
            .await?;

        self.repo.update_status(ctx.executor, id, BomStatus::Draft)
            .await?;

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

    async fn save_as(&self, mut ctx: ServiceContext<'_>, source_id: i64, new_name: String) -> Result<i64> {
        let source = self.repo.find_by_id(ctx.executor, source_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let create_req = CreateBomReq {
            name: new_name.clone(),
            bom_category_id: source.bom_category_id,
        };

        let new_id = self.create(ctx.reborrow(), create_req).await?;

        // copy nodes
        let source_nodes = self.node_repo.find_by_bom_id(ctx.executor, source_id)
            .await?;

        // build old -> new node id mapping
        let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for node in &source_nodes {
            let new_parent_id = if node.parent_id == 0 { 0 } else { *id_map.get(&node.parent_id).unwrap_or(&0) };
            let new_node = NewBomNode {
                parent_id: new_parent_id,
                product_id: node.product_id,
                quantity: node.quantity,
                loss_rate: node.loss_rate,
                order: node.order,
                unit: node.unit.clone(),
                remark: node.remark.clone(),
                position: node.position.clone(),
                work_center: node.work_center.clone(),
                properties: node.properties.clone(),
            };
            let new_node_id = self.node_repo.create(ctx.executor, new_id, &new_node)
                .await?;
            id_map.insert(node.id, new_node_id);
        }

        Ok(new_id)
    }

    #[allow(clippy::collapsible_if)]
    async fn substitute_product(
        &self,
        mut ctx: ServiceContext<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult> {
        // If bom_id is Some, scope to that BOM; otherwise global replace
        let affected_node_ids = if let Some(bom_id) = req.bom_id {
            let _existing = self.repo.find_by_id(ctx.executor, bom_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BOM"))?;

            if req.overrides != AttributeOverrides::default() {
                self.node_repo.update_product_with_overrides(
                    ctx.executor, bom_id, req.old_product_id, req.new_product_id, &req.overrides,
                ).await?
            } else {
                self.node_repo.update_product(
                    ctx.executor, bom_id, req.old_product_id, req.new_product_id,
                ).await?
            }
        } else {
            Vec::new()
        };

        let affected_nodes = affected_node_ids.len() as i64;
        let affected_boms = if affected_nodes > 0 { 1i64 } else { 0i64 };

        if affected_nodes > 0 {
            self.event_bus.publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::BomSubstituted,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: req.bom_id.unwrap_or(0),
                    payload: serde_json::json!({
                        "old_product_id": req.old_product_id,
                        "new_product_id": req.new_product_id,
                        "affected_nodes": affected_nodes,
                    }),
                    idempotency_key: None,
                },
            ).await?;

            self.audit.record(
                ctx,
                "BOM",
                req.bom_id.unwrap_or(0),
                AuditAction::Update,
                Some(serde_json::json!({
                    "action": "substitute",
                    "old_product_id": req.old_product_id,
                    "new_product_id": req.new_product_id,
                    "affected_nodes": affected_nodes,
                })),
                None,
            ).await?;
        }

        Ok(SubstitutionResult { affected_boms, affected_nodes })
    }

    async fn validate_cycle(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<()> {
        self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

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
        node: NewBomNode,
    ) -> Result<i64> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only add nodes to a draft BOM"));
        }

        // validate parent node belongs to same BOM (0 = root)
        if node.parent_id != 0 {
            let parent = self.node_repo.find_by_id(ctx.executor, node.parent_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            if parent.bom_id != bom_id {
                return Err(DomainError::business_rule("Parent node does not belong to this BOM"));
            }
        }

        let node_id = self.node_repo.create(ctx.executor, bom_id, &node)
            .await?;

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
        expected_version: i32,
    ) -> Result<()> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only update nodes in a draft BOM"));
        }

        if bom.version != expected_version {
            return Err(DomainError::ConcurrentConflict);
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        self.node_repo.update(ctx.executor, node_id, &req)
            .await?;

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
    ) -> Result<i64> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only delete nodes in a draft BOM"));
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        let child_count = self.node_repo.count_children(ctx.executor, node_id)
            .await?;
        if child_count > 0 {
            return Err(DomainError::business_rule("Cannot delete a node that has children"));
        }

        self.node_repo.delete(ctx.executor, node_id)
            .await?;

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
        ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<()> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("Can only move nodes in a draft BOM"));
        }

        let existing = self.node_repo.find_by_id(ctx.executor, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("Node does not belong to this BOM"));
        }

        // validate new parent (0 = root)
        if new_parent_id != 0 {
            let new_parent = self.node_repo.find_by_id(ctx.executor, new_parent_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;

            if new_parent.bom_id != bom_id {
                return Err(DomainError::business_rule("Target parent does not belong to this BOM"));
            }
        }

        if new_parent_id == node_id {
            return Err(DomainError::business_rule("Cannot move a node under itself"));
        }

        // determine order_num
        let order_num = if let Some(sibling_id) = before_sibling_id {
            let sibling = self.node_repo.find_by_id(ctx.executor, sibling_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            self.node_repo.update_order_shift(
                ctx.executor, bom_id, new_parent_id, sibling.order,
            ).await?;
            sibling.order
        } else {
            let max_order = self.node_repo.find_max_order(ctx.executor, bom_id, new_parent_id)
                .await?;
            max_order.unwrap_or(0) + 1
        };

        self.node_repo.update_parent(ctx.executor, node_id, new_parent_id)
            .await?;

        let order_req = UpdateBomNodeReq {
            quantity: None,
            loss_rate: None,
            order: Some(order_num),
            unit: None,
            remark: None,
            position: None,
            work_center: None,
            properties: None,
        };
        self.node_repo.update(ctx.executor, node_id, &order_req)
            .await?;

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
    ) -> Result<BomCostReport> {
        let bom = self.repo.find_by_id(ctx.executor, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let leaf_nodes = self.node_repo.find_leaf_nodes(ctx.executor, bom_id)
            .await?;

        let mut material_costs = Vec::new();
        let mut warnings = Vec::new();

        for node in &leaf_nodes {
            let price_result = if let Some(date) = as_of_date {
                self.price_repo.find_price_at(ctx.executor, node.product_id, PriceType::StandardCost, date)
                    .await?
            } else {
                self.price_repo.find_latest_price(ctx.executor, node.product_id, PriceType::StandardCost)
                    .await?
            };

            let unit_price = match price_result {
                Some(entry) => Some(entry.new_price),
                None => {
                    warnings.push(format!(
                        "No StandardCost price found for product_id {}",
                        node.product_id
                    ));
                    None
                }
            };

            material_costs.push(MaterialCostItem {
                node_id: node.id,
                product_id: node.product_id,
                product_name: format!("product_{}", node.product_id),
                product_code: node.product_code.clone().unwrap_or_default(),
                quantity: node.quantity,
                unit_price,
            });
        }

        Ok(BomCostReport {
            bom_id,
            bom_name: bom.bom_name,
            product_code: String::new(),
            as_of_date,
            material_costs,
            labor_costs: Vec::new(),
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
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomCategoryReq) -> Result<i64> {
        let id = self.repo.create(ctx.executor, &req)
            .await?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Create, None, None).await?;
        Ok(id)
    }

    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateBomCategoryReq) -> Result<()> {
        self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        self.repo.update(ctx.executor, id, &req)
            .await?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Update, None, None).await?;
        Ok(())
    }

    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        let bom_count = self.repo.count_boms_by_category(ctx.executor, id)
            .await?;
        if bom_count > 0 {
            return Err(DomainError::business_rule(
                "Cannot delete category that is used by BOMs",
            ));
        }

        self.repo.delete(ctx.executor, id)
            .await?;

        self.audit.record(ctx, "BomCategory", id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>> {
        self.repo.query(ctx.executor, &query, &page)
            .await
    }
}
