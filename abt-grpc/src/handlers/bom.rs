//! BOM gRPC Handler — delegated to abt-core BOM services

use crate::generated::abt::v1::{
    abt_bom_service_server::AbtBomService as GrpcBomService,
    Action, Resource,
    AddBomNodeRequest, BoolResponse, BomCostReportResponse, BomLaborCostResponse,
    BomListResponse, BomNodesResponse, BomResponse,
    CreateBomRequest, DeleteBomNodeRequest, DeleteBomRequest, DownloadBomRequest,
    DownloadFileResponse, ExistsBomNameRequest, ExportBomRequest, GetBomCostReportRequest,
    GetBomLaborCostRequest, GetBomRequest, GetLeafNodesRequest, GetProductCodeRequest,
    ListBomsRequest, PublishBomRequest, PublishBomResponse, SaveAsBomRequest,
    StringResponse, SubstituteProductRequest, SubstituteProductResponse, SwapBomNodeRequest,
    U64Response, UnpublishBomRequest, UnpublishBomResponse, UpdateBomNodeRequest,
    UpdateBomRequest,
};
use crate::handlers::{domain_to_status, validate_upload_path, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_core::master_data::bom::model::{
    AttributeOverrides, BomStatus, CreateBomReq, NewBomNode, SubstituteReq, UpdateBomNodeReq,
    UpdateBomReq, BomQuery,
};
use abt_core::master_data::bom::service::{
    BomCommandService, BomCostService, BomNodeService, BomQueryService,
};
use abt_core::shared::types::{PageParams, ServiceContext};
use abt_macros::require_permission;
use common::error;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

pub struct BomHandler;

impl BomHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BomHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcBomService for BomHandler {
    #[require_permission(Resource::Bom, Action::Read)]
    async fn list_boms(&self, request: Request<ListBomsRequest>) -> GrpcResult<BomListResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_query_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let status = req.status.and_then(|s| match s {
            1 => Some(BomStatus::Draft),
            2 => Some(BomStatus::Published),
            _ => None,
        });

        let query = BomQuery {
            name: req.keyword,
            status,
            bom_category_id: req.bom_category_id,
        };

        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(50));

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let result = srv.list(ctx, query, page).await.map_err(domain_to_status)?;

        Ok(Response::new(BomListResponse {
            items: result.items.into_iter().map(|b| b.into()).collect(),
            total: result.total as u64,
        }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_bom(&self, request: Request<GetBomRequest>) -> GrpcResult<BomResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_query_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let bom = srv.get(ctx, req.bom_id).await.map_err(domain_to_status)?;

        Ok(Response::new(bom.into()))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_bom(&self, request: Request<CreateBomRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv
            .create(
                ctx,
                CreateBomReq {
                    name: req.name,
                    bom_category_id: req.bom_category_id,
                },
            )
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_bom(&self, request: Request<UpdateBomRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        // Fetch current BOM to get version for optimistic concurrency
        let query_srv = state.bom_query_service();
        let mut ctx = ServiceContext::new(&mut tx, auth.user_id);
        let existing = query_srv.get(ctx.reborrow(), req.bom_id).await.map_err(domain_to_status)?;
        let expected_version = existing.version;

        srv.update(
            ctx,
            req.bom_id,
            UpdateBomReq {
                name: Some(req.name),
                bom_category_id: req.bom_category_id,
            },
            expected_version,
        )
        .await
        .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Delete)]
    async fn delete_bom(&self, request: Request<DeleteBomRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete(ctx, req.bom_id).await.map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn save_as_bom(&self, request: Request<SaveAsBomRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let new_id = srv
            .save_as(ctx, req.source_bom_id, req.new_name)
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response {
            value: new_id as u64,
        }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_product_code(
        &self,
        request: Request<GetProductCodeRequest>,
    ) -> GrpcResult<StringResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_query_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        // get_product_code not in abt-core service API — derive from leaf nodes
        let nodes = srv.get_leaf_nodes(ctx, req.bom_id).await.map_err(domain_to_status)?;
        let code = nodes
            .first()
            .and_then(|n| n.product_code.clone())
            .unwrap_or_default();

        Ok(Response::new(StringResponse { value: Some(code) }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn export_bom(&self, request: Request<ExportBomRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;

        // Verify BOM exists via abt-core query service
        let srv = state.bom_query_service();
        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.get(ctx, req.bom_id).await.map_err(domain_to_status)?;
        drop(tx);

        validate_upload_path(&req.file_path)?;

        // Excel export
        let exporter = abt_core::shared::excel::bom_export::BomExporter::new(
            state.core_pool(), req.bom_id,
        );
        let bytes: Vec<u8> = exporter
            .export()
            .await
            .map_err(error::err_to_status)?;

        tokio::fs::write(&req.file_path, bytes)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法写入导出文件: {}", e)))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_leaf_nodes(
        &self,
        request: Request<GetLeafNodesRequest>,
    ) -> GrpcResult<BomNodesResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_query_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let nodes = srv
            .get_leaf_nodes(ctx, req.bom_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(BomNodesResponse {
            items: nodes.into_iter().map(|n| n.into()).collect(),
        }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn add_bom_node(&self, request: Request<AddBomNodeRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_node_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let node = NewBomNode {
            product_id: req.product_id,
            quantity: rust_decimal::Decimal::from_f64_retain(req.quantity).unwrap_or_default(),
            parent_id: req.parent_id,
            loss_rate: rust_decimal::Decimal::from_f64_retain(req.loss_rate).unwrap_or_default(),
            order: 0, // auto-assigned by repo
            unit: if req.unit.is_empty() { None } else { Some(req.unit) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            position: if req.position.is_empty() { None } else { Some(req.position) },
            work_center: if req.work_center.is_empty() { None } else { Some(req.work_center) },
            properties: if req.properties.is_empty() { None } else { Some(req.properties) },
        };

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv
            .add_node(ctx, req.bom_id, node)
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_bom_node(
        &self,
        request: Request<UpdateBomNodeRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_node_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        // Fetch current BOM to get version for optimistic concurrency
        let query_srv = state.bom_query_service();
        let mut ctx = ServiceContext::new(&mut tx, auth.user_id);
        let bom = query_srv.get(ctx.reborrow(), req.bom_id).await.map_err(domain_to_status)?;
        let expected_version = bom.version;

        let update_req = UpdateBomNodeReq {
            quantity: Some(rust_decimal::Decimal::from_f64_retain(req.quantity).unwrap_or_default()),
            loss_rate: Some(rust_decimal::Decimal::from_f64_retain(req.loss_rate).unwrap_or_default()),
            order: None,
            unit: if req.unit.is_empty() { None } else { Some(req.unit) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            position: if req.position.is_empty() { None } else { Some(req.position) },
            work_center: if req.work_center.is_empty() { None } else { Some(req.work_center) },
            properties: if req.properties.is_empty() { None } else { Some(req.properties) },
        };

        srv.update_node(ctx, req.bom_id, req.node_id, update_req, expected_version)
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Delete)]
    async fn delete_bom_node(
        &self,
        request: Request<DeleteBomNodeRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_node_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let deleted_id = srv
            .delete_node(ctx, req.bom_id, req.node_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response {
            value: deleted_id as u64,
        }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn swap_bom_node(
        &self,
        request: Request<SwapBomNodeRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_node_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        // abt-core BomNodeService has move_node instead of swap_node_position.
        // To implement swap, we use two move_node calls:
        // 1. Move node_id_1 under node_id_2's parent at node_id_2's position
        // 2. Move node_id_2 under node_id_1's original parent at node_id_1's position
        // For now, we implement swap via the query service to find node parents,
        // then use move_node accordingly.

        let query_srv = state.bom_query_service();
        let mut ctx = ServiceContext::new(&mut tx, auth.user_id);

        // Verify BOM exists and get nodes from bom_detail
        let bom = query_srv.get(ctx.reborrow(), req.bom_id).await.map_err(domain_to_status)?;
        let nodes = &bom.bom_detail.nodes;

        let node1 = nodes.iter().find(|n| n.id == req.node_id_1)
            .ok_or_else(|| domain_to_status(abt_core::shared::types::DomainError::not_found("BomNode")))?;
        let node2 = nodes.iter().find(|n| n.id == req.node_id_2)
            .ok_or_else(|| domain_to_status(abt_core::shared::types::DomainError::not_found("BomNode")))?;

        // Swap: move node1 to node2's parent, node2 to node1's parent
        let node1_parent = node1.parent_id;
        let node2_parent = node2.parent_id;

        // Move node1 to node2's parent position
        srv.move_node(ctx.reborrow(), bom.bom_id, req.node_id_1, node2_parent, None)
            .await
            .map_err(domain_to_status)?;

        // Move node2 to node1's original parent
        srv.move_node(ctx, bom.bom_id, req.node_id_2, node1_parent, None)
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn exists_bom_name(
        &self,
        request: Request<ExistsBomNameRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_query_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let exists = srv
            .exists_name(ctx, &req.name, Some(auth.user_id))
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(BoolResponse { value: exists }))
    }

    type DownloadBomStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::Bom, Action::Read)]
    async fn download_bom(
        &self,
        request: Request<DownloadBomRequest>,
    ) -> Result<Response<Self::DownloadBomStream>, tonic::Status> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;

        // Verify BOM exists via abt-core query service
        let srv = state.bom_query_service();
        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let bom = srv.get(ctx, req.bom_id).await.map_err(domain_to_status)?;
        drop(tx);

        // Excel export
        let exporter = abt_core::shared::excel::bom_export::BomExporter::new(
            state.core_pool(), req.bom_id,
        );
        let (bytes, _bom_name): (Vec<u8>, String) = exporter
            .export_with_name()
            .await
            .map_err(error::err_to_status)?;

        let file_name = format!(
            "BOM_{}_{}.xlsx",
            bom.bom_name,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        Ok(Response::new(crate::handlers::stream_excel_bytes(file_name, bytes)))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn substitute_product(
        &self,
        request: Request<SubstituteProductRequest>,
    ) -> GrpcResult<SubstituteProductResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let overrides = AttributeOverrides {
            quantity: req.quantity.map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default()),
            loss_rate: req.loss_rate.map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default()),
            unit: if req.unit.as_ref().map_or(true, |s| s.is_empty()) { None } else { req.unit },
            remark: if req.remark.as_ref().map_or(true, |s| s.is_empty()) { None } else { req.remark },
            position: if req.position.as_ref().map_or(true, |s| s.is_empty()) { None } else { req.position },
            work_center: if req.work_center.as_ref().map_or(true, |s| s.is_empty()) { None } else { req.work_center },
            properties: if req.properties.as_ref().map_or(true, |s| s.is_empty()) { None } else { req.properties },
        };

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let result = srv
            .substitute_product(
                ctx,
                SubstituteReq {
                    old_product_id: req.old_product_id,
                    new_product_id: req.new_product_id,
                    bom_id: req.bom_id,
                    overrides,
                },
            )
            .await
            .map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(SubstituteProductResponse {
            affected_bom_count: result.affected_boms,
            replaced_node_count: result.affected_nodes,
        }))
    }

    #[require_permission(Resource::BomCost, Action::Read)]
    async fn get_bom_cost_report(
        &self,
        request: Request<GetBomCostReportRequest>,
    ) -> GrpcResult<BomCostReportResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_cost_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let report = srv
            .get_cost_report(ctx, req.bom_id, None)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(report.into()))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn publish_bom(
        &self,
        request: Request<PublishBomRequest>,
    ) -> GrpcResult<PublishBomResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let _id = srv.publish(ctx, req.bom_id).await.map_err(domain_to_status)?;

        // Fetch the updated BOM for response
        let query_srv = state.bom_query_service();
        let ctx2 = ServiceContext::new(&mut tx, auth.user_id);
        let bom = query_srv.get(ctx2, req.bom_id).await.map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(PublishBomResponse { bom: Some(bom.into()) }))
    }

    #[require_permission(Resource::BomLaborCost, Action::Read)]
    async fn get_bom_labor_cost(
        &self,
        request: Request<GetBomLaborCostRequest>,
    ) -> GrpcResult<BomLaborCostResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_cost_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        // abt-core has no dedicated get_bom_labor_cost — use cost report and extract labor_costs
        let report = srv
            .get_cost_report(ctx, req.bom_id, None)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(BomLaborCostResponse {
            bom_id: report.bom_id,
            bom_name: report.bom_name,
            product_code: report.product_code,
            labor_costs: report.labor_costs.into_iter().map(|l| l.into()).collect(),
            warnings: report.warnings,
        }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn unpublish_bom(
        &self,
        request: Request<UnpublishBomRequest>,
    ) -> GrpcResult<UnpublishBomResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_command_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.unpublish(ctx, req.bom_id).await.map_err(domain_to_status)?;

        // Fetch the updated BOM for response
        let query_srv = state.bom_query_service();
        let ctx2 = ServiceContext::new(&mut tx, auth.user_id);
        let bom = query_srv.get(ctx2, req.bom_id).await.map_err(domain_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(UnpublishBomResponse { bom: Some(bom.into()) }))
    }
}
