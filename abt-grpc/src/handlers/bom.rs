//! BOM gRPC Handler

use crate::generated::abt::v1::{abt_bom_service_server::AbtBomService as GrpcBomService, *};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use std::path::Path;
use tokio_stream::wrappers::ReceiverStream;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "delete").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "delete").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
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
        let auth = extract_auth(&request)?;
        auth.check_permission("labor_process", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
        crate::handlers::labor_process::list_labor_processes_internal(request.into_inner()).await
    }

    async fn create_labor_process(
        &self,
        request: Request<CreateLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        auth.check_permission("labor_process", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
        crate::handlers::labor_process::create_labor_process_internal(request.into_inner()).await
    }

    async fn update_labor_process(
        &self,
        request: Request<UpdateLaborProcessRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("labor_process", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
        crate::handlers::labor_process::update_labor_process_internal(request.into_inner()).await
    }

    async fn delete_labor_process(
        &self,
        request: Request<DeleteLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        auth.check_permission("labor_process", "delete").map_err(|e| Status::permission_denied(e.to_string()))?;
        crate::handlers::labor_process::delete_labor_process_internal(request.into_inner()).await
    }

    async fn import_labor_processes(
        &self,
        request: Request<ImportLaborProcessRequest>,
    ) -> GrpcResult<ImportLaborProcessResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("labor_process", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
        crate::handlers::labor_process::import_labor_processes_internal(request.into_inner()).await
    }

    type DownloadBomStream = ReceiverStream<Result<DownloadFileResponse, Status>>;

    async fn download_bom(
        &self,
        request: Request<DownloadBomRequest>,
    ) -> Result<Response<Self::DownloadBomStream>, Status> {
        let auth = extract_auth(&request)?;
        auth.check_permission("bom", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        // 生成 Excel 到内存，同时获取 BOM 名称
        let (bytes, bom_name) = srv
            .export_to_bytes(req.bom_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let file_name = format!(
            "BOM_{}_{}.xlsx",
            bom_name,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        let file_size = bytes.len() as i64;

        // 创建流式响应
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            // 发送元数据
            let metadata = FileMetadata {
                file_name,
                file_size,
                content_type: crate::handlers::EXCEL_MIME_TYPE.to_string(),
            };
            let first_msg = DownloadFileResponse {
                data: Some(download_file_response::Data::Metadata(metadata)),
            };
            if tx.send(Ok(first_msg)).await.is_err() {
                return;
            }

            // 分块发送文件内容
            for chunk in bytes.chunks(crate::handlers::STREAM_CHUNK_SIZE) {
                let chunk_msg = DownloadFileResponse {
                    data: Some(download_file_response::Data::Chunk(chunk.to_vec())),
                };
                if tx.send(Ok(chunk_msg)).await.is_err() {
                    return;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
