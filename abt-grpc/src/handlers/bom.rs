//! BOM gRPC Handler

use crate::generated::abt::v1::{abt_bom_service_server::AbtBomService as GrpcBomService, *};
use crate::handlers::{validate_upload_path, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use common::error;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

use abt::{BomService, ExcelExportService, ExportRequest};

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
        let srv = state.bom_service();

        let status = req.status.and_then(|s| match s {
            1 => Some(abt::BomStatus::Draft),
            2 => Some(abt::BomStatus::Published),
            _ => None,
        });

        let query = abt::BomQuery {
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
            bom_name: req.keyword,
            product_code: req.product_code,
            date_from: req.date_from,
            date_to: req.date_to,
            bom_category_id: req.bom_category_id,
            status,
            caller_id: Some(auth.user_id),
            ..Default::default()
        };

        let (items, total) = srv
            .query(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomListResponse {
            items: items.into_iter().map(|b| b.into()).collect(),
            total: total as u64,
        }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_bom(&self, request: Request<GetBomRequest>) -> GrpcResult<BomResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        Ok(Response::new(bom.into()))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_bom(&self, request: Request<CreateBomRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let user_id = auth.user_id;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let id = srv
            .create(&req.name, user_id, req.bom_category_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        srv.update_metadata(req.bom_id, &req.name, req.bom_category_id, auth.user_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        srv.delete(req.bom_id, auth.user_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn save_as_bom(&self, request: Request<SaveAsBomRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let user_id = auth.user_id;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let new_id = srv
            .save_as(req.source_bom_id, &req.new_name, user_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        let code = srv
            .get_product_code(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(StringResponse { value: code }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn export_bom(&self, request: Request<ExportBomRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;

        let bom = abt::repositories::BomRepo::find_by_id_pool(&state.pool(), req.bom_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        validate_upload_path(&req.file_path)?;

        let exporter = abt::excel::BomExporter::new(state.pool());
        let bytes = exporter
            .export(ExportRequest { params: req.bom_id })
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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        let nodes = srv
            .get_leaf_nodes(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomNodesResponse {
            items: nodes.into_iter().map(|n| n.into()).collect(),
        }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn add_bom_node(&self, request: Request<AddBomNodeRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        let node = abt::BomNode {
            id: 0,
            product_id: req.product_id,
            quantity: req.quantity,
            parent_id: req.parent_id,
            loss_rate: req.loss_rate,
            unit: Some(req.unit),
            remark: Some(req.remark),
            position: Some(req.position),
            work_center: Some(req.work_center),
            properties: Some(req.properties),
            product_code: None,
            order: 0,
        };

        let id = srv
            .add_node(req.bom_id, node, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        let node = abt::BomNode {
            id: req.node_id,
            quantity: req.quantity,
            loss_rate: req.loss_rate,
            unit: Some(req.unit),
            remark: Some(req.remark),
            position: Some(req.position),
            work_center: Some(req.work_center),
            properties: Some(req.properties),
            ..Default::default()
        };

        srv.update_node(req.bom_id, node, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        let deleted_count = srv
            .delete_node(req.bom_id, req.node_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response {
            value: deleted_count as u64,
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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, true)
            .map_err(|e| error::err_to_status(e))?;

        srv.swap_node_position(req.bom_id, req.node_id_1, req.node_id_2, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = AppState::get().await.bom_service();

        let exists = srv
            .exists_name(&req.name, Some(auth.user_id))
            .await
            .map_err(error::err_to_status)?;

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

        let bom = abt::repositories::BomRepo::find_by_id_pool(&state.pool(), req.bom_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        // 导出 Excel 同时获取 BOM 名称
        let exporter = abt::excel::BomExporter::new(state.pool());
        let (bytes, bom_name) = exporter
            .export_with_name(req.bom_id)
            .await
            .map_err(error::err_to_status)?;

        let file_name = format!(
            "BOM_{}_{}.xlsx",
            bom_name,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        Ok(Response::new(crate::handlers::stream_excel_bytes(file_name, bytes)))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn substitute_product(
        &self,
        request: Request<SubstituteProductRequest>,
    ) -> GrpcResult<SubstituteProductResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let overrides = abt::AttributeOverrides {
            quantity: req.quantity,
            loss_rate: req.loss_rate,
            unit: req.unit,
            remark: req.remark,
            position: req.position,
            work_center: req.work_center,
            properties: req.properties,
        };

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let (affected_bom_count, replaced_node_count) = srv
            .substitute_product(
                req.old_product_id,
                req.new_product_id,
                req.bom_id,
                overrides,
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(SubstituteProductResponse {
            affected_bom_count,
            replaced_node_count,
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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BOM", &req.bom_id.to_string()))?;

        bom.require_creator_or_published(auth.user_id, false)
            .map_err(|_| error::not_found("BOM", &req.bom_id.to_string()))?;

        let report = srv
            .get_bom_cost_report(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let bom = srv
            .publish(req.bom_id, auth.user_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(PublishBomResponse { bom: Some(bom.into()) }))
    }
}
