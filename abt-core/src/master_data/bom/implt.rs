use sqlx::PgPool;

use super::model::*;
use super::repo::{BomCategoryRepo, BomNodeRepo, BomRepo, BomSnapshotRepo};
use super::service::{
    BomCategoryService, BomCommandService, BomCostService, BomNodeService, BomQueryService,
};
use crate::master_data::price::model::PriceType;
use crate::master_data::price::repo::PriceRepo;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

// ── BomQueryServiceImpl ──────────────────────────────────────────────────────

pub struct BomQueryServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
}

impl BomQueryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        let _ = pool;
        Self { repo: BomRepo, node_repo: BomNodeRepo, snapshot_repo: BomSnapshotRepo }
    }
}

#[async_trait::async_trait]
impl BomQueryService for BomQueryServiceImpl {
    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, bom_id: i64) -> Result<Bom> {
        let mut bom = self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;
        let nodes = self.node_repo.find_by_bom_id(db, bom_id)
            .await?;
        bom.bom_detail = BomDetail { nodes };
        Ok(bom)
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>> {
        self.repo.query(db, &query, &page)
            .await
    }

    async fn get_leaf_nodes(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>> {
        self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        self.node_repo.find_leaf_nodes(db, bom_id)
            .await
    }

    async fn get_snapshots(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>> {
        if let Some(ver) = version {
            let snap = self.snapshot_repo.find_by_bom_and_version(db, bom_id, ver)
                .await?;
            Ok(snap.into_iter().collect())
        } else {
            self.snapshot_repo.find_by_bom_id(db, bom_id, limit)
                .await
        }
    }

    async fn exists_name(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool> {
        self.repo.check_name_unique(db, name, caller_id)
            .await
    }
}

// ── BomCommandServiceImpl ────────────────────────────────────────────────────

pub struct BomCommandServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    snapshot_repo: BomSnapshotRepo,
    pool: PgPool,
}

impl BomCommandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomRepo, node_repo: BomNodeRepo, snapshot_repo: BomSnapshotRepo, pool }
    }
}

#[async_trait::async_trait]
impl BomCommandService for BomCommandServiceImpl {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomReq) -> Result<i64> {
        req.bom_category_id
            .ok_or_else(|| DomainError::validation("请选择BOM分类"))?;

        if !self.repo.check_name_unique(db, &req.name, None)
            .await?
        {
            return Err(DomainError::duplicate(format!("BOM name: {}", req.name)));
        }

        let id = self.repo.create(db, &req, ctx.operator_id)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "BomStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BOM", entity_id: id, action: AuditAction::Create, changes: None, context: None }).await?;

        Ok(id)
    }

    #[allow(clippy::collapsible_if)]
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Draft {
            return Err(DomainError::business_rule("已发布的 BOM 无法修改"));
        }

        if let Some(ref new_name) = req.name {
            if !self.repo.check_name_unique(db, new_name, Some(id))
                .await?
            {
                return Err(DomainError::duplicate(format!("BOM name: {new_name}")));
            }
        }

        let updated = self.repo.update(db, id, &req, expected_version)
            .await?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BOM", entity_id: id, action: AuditAction::Update, changes: None, context: None }).await?;
        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("已发布的 BOM 无法删除"));
        }

        self.repo.delete(db, id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BOM", entity_id: id, action: AuditAction::Delete, changes: None, context: None }).await?;
        Ok(())
    }

    async fn publish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<i64> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status == BomStatus::Published {
            return Err(DomainError::business_rule("BOM 已处于发布状态"));
        }

        let node_count = self.node_repo.count_by_bom_id(db, id)
            .await?;
        if node_count == 0 {
            return Err(DomainError::business_rule("BOM 没有节点，无法发布"));
        }

        // build BomDetail from nodes
        let nodes = self.node_repo.find_by_bom_id(db, id)
            .await?;
        let bom_detail = BomDetail { nodes };

        self.snapshot_repo.create(
            db, id, existing.version, &existing.bom_name, &bom_detail, ctx.operator_id,
        ).await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "BomStatus", id, "Published", None)
            .await?;

        self.repo.update_status(db, id, BomStatus::Published, Some(chrono::Utc::now()))
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::BomPublished,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "bom_id": id, "version": existing.version }),
                    idempotency_key: None,
                },
            ).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BOM", entity_id: id, action: AuditAction::Transition, changes: None, context: None }).await?;

        Ok(id)
    }

    async fn unpublish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if existing.status != BomStatus::Published {
            return Err(DomainError::business_rule("BOM 未发布"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "BomStatus", id, "Draft", None)
            .await?;

        self.repo.update_status(db, id, BomStatus::Draft, None)
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::BomUnpublished,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "bom_id": id }),
                    idempotency_key: None,
                },
            ).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BOM", entity_id: id, action: AuditAction::Transition, changes: None, context: None }).await?;
        Ok(())
    }

    async fn save_as(&self, ctx: &ServiceContext, db: PgExecutor<'_>, source_id: i64, new_name: String) -> Result<i64> {
        let source = self.repo.find_by_id(db, source_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let create_req = CreateBomReq {
            name: new_name.clone(),
            bom_category_id: source.bom_category_id,
        };

        let new_id = self.create(ctx, db, create_req).await?;

        // copy nodes
        let source_nodes = self.node_repo.find_by_bom_id(db, source_id)
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
            let new_node_id = self.node_repo.create(db, new_id, &new_node)
                .await?;
            id_map.insert(node.id, new_node_id);
        }

        Ok(new_id)
    }

    #[allow(clippy::collapsible_if)]
    async fn substitute_product(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult> {
        // If bom_id is Some, scope to that BOM; otherwise global replace
        let affected_node_ids = if let Some(bom_id) = req.bom_id {
            let _existing = self.repo.find_by_id(db, bom_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BOM"))?;

            if req.overrides != AttributeOverrides::default() {
                self.node_repo.update_product_with_overrides(
                    db, bom_id, req.old_product_id, req.new_product_id, &req.overrides,
                ).await?
            } else {
                self.node_repo.update_product(
                    db, bom_id, req.old_product_id, req.new_product_id,
                ).await?
            }
        } else {
            Vec::new()
        };

        let affected_nodes = affected_node_ids.len() as i64;
        let affected_boms = if affected_nodes > 0 { 1i64 } else { 0i64 };

        if affected_nodes > 0 {
            new_domain_event_bus(self.pool.clone())
                .publish(
                    ctx, db,
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

            new_audit_log_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "BOM",
                        entity_id: req.bom_id.unwrap_or(0),
                        action: AuditAction::Update,
                        changes: Some(serde_json::json!({
                            "action": "substitute",
                            "old_product_id": req.old_product_id,
                            "new_product_id": req.new_product_id,
                            "affected_nodes": affected_nodes,
                        })),
                        context: None,
                    },
                ).await?;
        }

        Ok(SubstitutionResult { affected_boms, affected_nodes })
    }

    async fn validate_cycle(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, bom_id: i64) -> Result<()> {
        self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        Ok(())
    }
}

// ── BomNodeServiceImpl ───────────────────────────────────────────────────────

pub struct BomNodeServiceImpl {
    repo: BomRepo,
    node_repo: BomNodeRepo,
    pool: PgPool,
}

impl BomNodeServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomRepo, node_repo: BomNodeRepo, pool }
    }
}

#[async_trait::async_trait]
impl BomNodeService for BomNodeServiceImpl {
    async fn add_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node: NewBomNode,
    ) -> Result<i64> {
        let bom = self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的 BOM 可以添加节点"));
        }

        // validate parent node belongs to same BOM (0 = root)
        if node.parent_id != 0 {
            let parent = self.node_repo.find_by_id(db, node.parent_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            if parent.bom_id != bom_id {
            return Err(DomainError::business_rule("父节点不属于当前 BOM"));
            }
        }

        let node_id = self.node_repo.create(db, bom_id, &node)
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
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

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomNode", entity_id: node_id, action: AuditAction::Create, changes: None, context: None }).await?;

        Ok(node_id)
    }

    async fn update_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        req: UpdateBomNodeReq,
        expected_version: i32,
    ) -> Result<()> {
        let bom = self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的 BOM 可以修改节点"));
        }

        if bom.version != expected_version {
            return Err(DomainError::ConcurrentConflict);
        }

        let existing = self.node_repo.find_by_id(db, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("节点不属于当前 BOM"));
        }

        self.node_repo.update(db, node_id, &req)
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::BomNodeUpdated,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: bom_id,
                    payload: serde_json::json!({ "node_id": node_id }),
                    idempotency_key: None,
                },
            ).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomNode", entity_id: node_id, action: AuditAction::Update, changes: None, context: None }).await?;
        Ok(())
    }

    async fn delete_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
    ) -> Result<i64> {
        let bom = self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        if bom.status != BomStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的 BOM 可以删除节点"));
        }

        let existing = self.node_repo.find_by_id(db, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("节点不属于当前 BOM"));
        }

        let child_count = self.node_repo.count_children(db, node_id)
            .await?;
        if child_count > 0 {
            return Err(DomainError::business_rule("存在子节点，无法删除"));
        }

        self.node_repo.delete(db, node_id)
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::BomNodeDeleted,
                    aggregate_type: "BOM".to_string(),
                    aggregate_id: bom_id,
                    payload: serde_json::json!({ "node_id": node_id }),
                    idempotency_key: None,
                },
            ).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomNode", entity_id: node_id, action: AuditAction::Delete, changes: None, context: None }).await?;

        Ok(node_id)
    }

    async fn move_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<()> {

        let existing = self.node_repo.find_by_id(db, node_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomNode"))?;

        if existing.bom_id != bom_id {
            return Err(DomainError::business_rule("节点不属于当前 BOM"));
        }

        // validate new parent (0 = root)
        if new_parent_id != 0 {
            let new_parent = self.node_repo.find_by_id(db, new_parent_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;

            if new_parent.bom_id != bom_id {
            return Err(DomainError::business_rule("目标父节点不属于当前 BOM"));
            }
        }

        if new_parent_id == node_id {
            return Err(DomainError::business_rule("不能将节点移动到自身下"));
        }

        // determine order_num
        let order_num = if let Some(sibling_id) = before_sibling_id {
            let sibling = self.node_repo.find_by_id(db, sibling_id)
                .await?
                .ok_or_else(|| DomainError::not_found("BomNode"))?;
            self.node_repo.update_order_shift(
                db, bom_id, new_parent_id, sibling.order,
            ).await?;
            sibling.order
        } else {
            let max_order = self.node_repo.find_max_order(db, bom_id, new_parent_id)
                .await?;
            max_order.unwrap_or(0) + 1
        };

        self.node_repo.update_parent(db, node_id, new_parent_id)
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
        self.node_repo.update(db, node_id, &order_req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomNode",
                    entity_id: node_id,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "action": "move",
                        "new_parent_id": new_parent_id,
                        "order_num": order_num,
                    })),
                    context: None,
                },
            ).await?;

        Ok(())
    }
}

// ── BomCostServiceImpl ───────────────────────────────────────────────────────

pub struct BomCostServiceImpl {
    pool: PgPool,
    repo: BomRepo,
    node_repo: BomNodeRepo,
    price_repo: PriceRepo,
}

impl BomCostServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, repo: BomRepo, node_repo: BomNodeRepo, price_repo: PriceRepo }
    }
}

#[async_trait::async_trait]
impl BomCostService for BomCostServiceImpl {
    async fn get_cost_report(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        as_of_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<BomCostReport> {
        use crate::master_data::product::{new_product_service, service::ProductService};
        use crate::master_data::bom_labor_process::repo::BomLaborProcessRepo;

        let bom = self.repo.find_by_id(db, bom_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BOM"))?;

        let leaf_nodes = self.node_repo.find_leaf_nodes(db, bom_id)
            .await?;

        // Resolve product names
        let product_ids: Vec<i64> = leaf_nodes.iter().map(|n| n.product_id).collect();
        let product_svc = new_product_service(self.pool.clone());
        let products = if product_ids.is_empty() {
            Vec::new()
        } else {
            product_svc.get_by_ids(_ctx, db, product_ids).await.unwrap_or_default()
        };
        let product_map: std::collections::HashMap<i64, String> = products.iter()
            .map(|p| (p.product_id, p.pdt_name.clone()))
            .collect();

        // Root product_code for labor cost lookup — may be NULL on the node, resolve from product table
        let root_node = self.node_repo.find_root_node(db, bom_id).await?;
        let root_product_code = match root_node.as_ref().and_then(|n| n.product_code.clone()) {
            Some(code) if !code.is_empty() => code,
            _ => {
                // Fallback: resolve product_code from the root node's product_id
                if let Some(ref rn) = root_node {
                    product_svc.get(_ctx, db, rn.product_id).await
                        .map(|p| p.product_code)
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
        };

        // Material costs
        let mut material_costs = Vec::new();
        let mut warnings = Vec::new();

        for node in &leaf_nodes {
            let price_result = if let Some(date) = as_of_date {
                self.price_repo.find_price_at(db, node.product_id, PriceType::Purchase, date)
                    .await?
            } else {
                self.price_repo.find_latest_price(db, node.product_id, PriceType::Purchase)
                    .await?
            };

            let unit_price = match price_result {
                Some(entry) => Some(entry.new_price),
                None => {
                    warnings.push(format!(
                        "产品 {} ({}) 缺失单价",
                        product_map.get(&node.product_id).map(|s| s.as_str()).unwrap_or("-"),
                        node.product_code.as_deref().unwrap_or("-")
                    ));
                    None
                }
            };

            material_costs.push(MaterialCostItem {
                node_id: node.id,
                product_id: node.product_id,
                product_name: product_map.get(&node.product_id).cloned().unwrap_or_else(|| format!("product_{}", node.product_id)),
                product_code: node.product_code.clone().unwrap_or_default(),
                quantity: node.quantity,
                unit_price,
            });
        }

        // Labor costs
        let labor_repo = BomLaborProcessRepo;
        let labor_rows: Vec<crate::master_data::bom_labor_process::BomLaborProcess> = if root_product_code.is_empty() {
            Vec::new()
        } else {
            labor_repo.find_all_by_product_code(db, &root_product_code).await.unwrap_or_default()
        };
        let labor_costs: Vec<LaborCostItem> = labor_rows.iter().map(|r| LaborCostItem {
            id: r.id,
            name: r.name.clone(),
            unit_price: r.unit_price,
            quantity: r.quantity,
            sort_order: r.sort_order,
            remark: r.remark.clone().unwrap_or_default(),
        }).collect();

        Ok(BomCostReport {
            bom_id,
            bom_name: bom.bom_name,
            product_code: root_product_code,
            as_of_date,
            material_costs,
            labor_costs,
            warnings,
        })
    }

    async fn get_labor_cost_report(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<BomLaborCostReport> {
        use crate::master_data::bom_labor_process::repo::BomLaborProcessRepo;
        use crate::master_data::product::{new_product_service, service::ProductService};

        let root_node = self.node_repo.find_root_node(db, bom_id).await?
            .ok_or_else(|| DomainError::not_found("BOM root node"))?;

        // product_code on the node may be NULL — resolve from product table
        let product_code = match root_node.product_code.clone() {
            Some(code) if !code.is_empty() => code,
            _ => {
                let product_svc = new_product_service(self.pool.clone());
                let product = product_svc.get(_ctx, db, root_node.product_id).await?;
                product.product_code
            }
        };

        let labor_repo = BomLaborProcessRepo;
        let rows: Vec<crate::master_data::bom_labor_process::BomLaborProcess> = labor_repo.find_all_by_product_code(db, &product_code).await?;

        let items: Vec<LaborCostItem> = rows.iter().map(|r| LaborCostItem {
            id: r.id,
            name: r.name.clone(),
            unit_price: r.unit_price,
            quantity: r.quantity,
            sort_order: r.sort_order,
            remark: r.remark.clone().unwrap_or_default(),
        }).collect();

        let total_cost = items.iter()
            .fold(rust_decimal::Decimal::ZERO, |acc, item| acc + item.unit_price * item.quantity);

        Ok(BomLaborCostReport {
            bom_id,
            items,
            total_cost,
        })
    }
}

// ── BomCategoryServiceImpl ───────────────────────────────────────────────────

pub struct BomCategoryServiceImpl {
    repo: BomCategoryRepo,
    pool: PgPool,
}

impl BomCategoryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomCategoryRepo, pool }
    }
}

#[async_trait::async_trait]
impl BomCategoryService for BomCategoryServiceImpl {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomCategoryReq) -> Result<i64> {
        let id = self.repo.create(db, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomCategory", entity_id: id, action: AuditAction::Create, changes: None, context: None }).await?;
        Ok(id)
    }

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateBomCategoryReq) -> Result<()> {
        self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        self.repo.update(db, id, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomCategory", entity_id: id, action: AuditAction::Update, changes: None, context: None }).await?;
        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomCategory"))?;

        let bom_count = self.repo.count_boms_by_category(db, id)
            .await?;
        if bom_count > 0 {
            return Err(DomainError::business_rule(
                "该分类下存在关联 BOM，无法删除",
            ));
        }

        self.repo.delete(db, id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomCategory", entity_id: id, action: AuditAction::Delete, changes: None, context: None }).await?;
        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>> {
        self.repo.query(db, &query, &page)
            .await
    }
}
