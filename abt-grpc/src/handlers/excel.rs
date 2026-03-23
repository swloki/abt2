//! Excel gRPC Handler

use std::path::Path;
use tonic::{Request, Response, Status, Streaming};
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
    async fn upload_file(
        &self,
        request: Request<Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, Status> {
        let config = crate::server::get_config();
        let upload_dir = Path::new(&config.upload_temp_dir);

        // 确保上传目录存在
        tokio::fs::create_dir_all(upload_dir)
            .await
            .map_err(|e| Status::internal(format!("Failed to create upload dir: {}", e)))?;

        let mut stream = request.into_inner();
        let mut file_name = String::new();
        let mut file_path: Option<std::path::PathBuf> = None;
        let mut file: Option<tokio::fs::File> = None;
        let mut total_size: i64 = 0;

        use futures::StreamExt;

        while let Some(message) = stream.next().await {
            let msg = message.map_err(|e| Status::internal(format!("Stream error: {}", e)))?;

            match msg.data {
                Some(upload_file_request::Data::FileName(name)) => {
                    file_name = name;
                    // 生成唯一文件名
                    let unique_name = format!("{}_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), file_name);
                    let path = upload_dir.join(&unique_name);
                    file_path = Some(path.clone());

                    file = Some(tokio::fs::File::create(&path)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to create file: {}", e)))?);
                }
                Some(upload_file_request::Data::Chunk(chunk)) => {
                    if let Some(ref mut f) = file {
                        use tokio::io::AsyncWriteExt;
                        f.write_all(&chunk)
                            .await
                            .map_err(|e| Status::internal(format!("Failed to write chunk: {}", e)))?;
                        total_size += chunk.len() as i64;
                    } else {
                        return Err(Status::failed_precondition("File name must be sent first"));
                    }
                }
                None => {}
            }
        }

        // 确保文件已刷新到磁盘
        if let Some(ref mut f) = file {
            use tokio::io::AsyncWriteExt;
            f.flush()
                .await
                .map_err(|e| Status::internal(format!("Failed to flush file: {}", e)))?;
        }

        let file_path = file_path.ok_or_else(|| Status::invalid_argument("No file uploaded"))?;

        // 返回相对路径
        let relative_path = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_name)
            .to_string();

        Ok(Response::new(UploadFileResponse {
            file_path: relative_path,
            file_size: total_size,
        }))
    }

    async fn import_excel(
        &self,
        request: Request<ImportExcelRequest>,
    ) -> GrpcResult<ImportResultResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.excel_service();
        let config = crate::server::get_config();

        let path = Path::new(&config.upload_temp_dir).join(&req.file_path);
        let operator_id = req.operator_id;

        let result = srv.import_quantity_from_excel(&state.pool(), &path, operator_id).await
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
