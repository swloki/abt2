//! BOM gRPC Handler

use crate::generated::abt::v1::{abt_bom_service_server::AbtBomService as GrpcBomService, *};
use crate::handlers::GrpcResult;
use crate::server::AppState;
use std::path::Path;
use tonic::{Request, Response, Status};

// Import trait to bring methods into scope
use abt::BomService;

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
    async fn list_boms(&self, request: Request<ListBomsRequest>) -> GrpcResult<BomListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let query = abt::BomQuery {
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
            bom_name: req.keyword,
            product_code: req.product_code,
            date_from: req.date_from,
            date_to: req.date_to,
            ..Default::default()
        };

        let (items, total) = srv
            .query(query)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BomListResponse {
            items: items.into_iter().map(|b| b.into()).collect(),
            total: total as u64,
        }))
    }

    async fn get_bom(&self, request: Request<GetBomRequest>) -> GrpcResult<BomResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let bom = srv
            .find(req.bom_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("BOM not found"))?;

        Ok(Response::new(bom.into()))
    }

    async fn create_bom(&self, request: Request<CreateBomRequest>) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let id = srv
            .create(&req.name, &req.created_by, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_bom(&self, request: Request<UpdateBomRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.update_name(req.bom_id, &req.name, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_bom(&self, request: Request<DeleteBomRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(req.bom_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn save_as_bom(&self, request: Request<SaveAsBomRequest>) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let new_id = srv
            .save_as(req.source_bom_id, &req.new_name, &req.created_by, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response {
            value: new_id as u64,
        }))
    }

    async fn get_product_code(
        &self,
        request: Request<GetProductCodeRequest>,
    ) -> GrpcResult<StringResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let code = srv
            .get_product_code(req.bom_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StringResponse { value: code }))
    }

    async fn export_bom(&self, request: Request<ExportBomRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let path = Path::new(&req.file_path);
        srv.export_to_excel(req.bom_id, path)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_leaf_nodes(
        &self,
        request: Request<GetLeafNodesRequest>,
    ) -> GrpcResult<BomNodesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let nodes = srv
            .get_leaf_nodes(req.bom_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BomNodesResponse {
            items: nodes.into_iter().map(|n| n.into()).collect(),
        }))
    }

    async fn add_bom_node(&self, request: Request<AddBomNodeRequest>) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_bom_node(
        &self,
        request: Request<UpdateBomNodeRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_bom_node(
        &self,
        request: Request<DeleteBomNodeRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let deleted_count = srv
            .delete_node(req.bom_id, req.node_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response {
            value: deleted_count as u64,
        }))
    }

    async fn swap_bom_node(
        &self,
        request: Request<SwapBomNodeRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.swap_node_position(req.bom_id, req.node_id_1, req.node_id_2, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn exists_bom_name(
        &self,
        request: Request<ExistsBomNameRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let srv = AppState::get().await.bom_service();

        let exists = srv
            .exists_name(&req.name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: exists }))
    }

    async fn list_labor_processes(
        &self,
        request: Request<ListLaborProcessesRequest>,
    ) -> GrpcResult<BomLaborProcessListResponse> {
        crate::handlers::labor_process::list_labor_processes_internal(request.into_inner()).await
    }

    async fn create_labor_process(
        &self,
        request: Request<CreateLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        crate::handlers::labor_process::create_labor_process_internal(request.into_inner()).await
    }

    async fn update_labor_process(
        &self,
        request: Request<UpdateLaborProcessRequest>,
    ) -> GrpcResult<BoolResponse> {
        crate::handlers::labor_process::update_labor_process_internal(request.into_inner()).await
    }

    async fn delete_labor_process(
        &self,
        request: Request<DeleteLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        crate::handlers::labor_process::delete_labor_process_internal(request.into_inner()).await
    }

    async fn import_labor_processes(
        &self,
        request: Request<ImportLaborProcessRequest>,
    ) -> GrpcResult<ImportLaborProcessResponse> {
        crate::handlers::labor_process::import_labor_processes_internal(request.into_inner()).await
    }
}
