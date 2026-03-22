//! Excel gRPC Handler

use std::path::Path;
use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{abt_excel_service_server::AbtExcelService as GrpcExcelService, *};
use crate::handlers::GrpcResult;
use crate::server::AppState;

// Import trait to bring methods into scope
use abt::ProductExcelService;

pub struct ExcelHandler;

impl ExcelHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExcelHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcExcelService for ExcelHandler {
    async fn import_excel(
        &self,
        request: Request<ImportExcelRequest>,
    ) -> GrpcResult<ImportResultResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.excel_service();

        let path = Path::new(&req.file_path);
        let operator_id = req.operator_id;

        let result = srv.import_quantity_from_excel(&state.pool(), path, operator_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ImportResultResponse {
            success_count: result.success_count as i32,
            failed_count: result.failed_count as i32,
            errors: result.errors,
        }))
    }

    async fn export_excel(&self, request: Request<ExportExcelRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.excel_service();

        let path = Path::new(&req.file_path);
        srv.export_products_to_excel(&state.pool(), path).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_progress(&self, _request: Request<Empty>) -> GrpcResult<ExcelProgressResponse> {
        let state = AppState::get().await;
        let srv = state.excel_service();

        let progress = srv.get_progress();

        Ok(Response::new(ExcelProgressResponse {
            current: progress.current as i32,
            total: progress.total as i32,
        }))
    }
}
