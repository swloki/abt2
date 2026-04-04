//! Excel gRPC Handler

use std::path::Path;
use tokio_stream::wrappers::ReceiverStream;
use common::error;
use tonic::{Request, Response, Streaming};
use crate::generated::abt::v1::{abt_excel_service_server::AbtExcelService as GrpcExcelService, *};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;

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
    #[require_permission("excel", "write")]
    async fn upload_file(
        &self,
        request: Request<Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, tonic::Status> {
        let upload_dir = Path::new("/tmp");

        // 确保上传目录存在
        tokio::fs::create_dir_all(upload_dir)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to create upload dir: {}", e)))?;

        let mut stream = request.into_inner();
        let mut file_name: String;
        let mut file_path: Option<std::path::PathBuf> = None;
        let mut file: Option<tokio::fs::File> = None;
        let mut total_size: i64 = 0;

        use futures::StreamExt;

        while let Some(message) = stream.next().await {
            let msg = message.map_err(|e| error::err_to_status(anyhow::anyhow!("Stream error: {}", e)))?;

            match msg.data {
                Some(upload_file_request::Data::FileName(name)) => {
                    file_name = name;
                    // 生成唯一文件名
                    let unique_name = format!("{}_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), file_name);
                    let path = upload_dir.join(&unique_name);
                    file_path = Some(path.clone());

                    file = Some(tokio::fs::File::create(&path)
                        .await
                        .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to create file: {}", e)))?);
                }
                Some(upload_file_request::Data::Chunk(chunk)) => {
                    if let Some(ref mut f) = file {
                        use tokio::io::AsyncWriteExt;
                        f.write_all(&chunk)
                            .await
                            .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to write chunk: {}", e)))?;
                        total_size += chunk.len() as i64;
                    } else {
                        return Err(tonic::Status::failed_precondition("File name must be sent first"));
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
                .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to flush file: {}", e)))?;
        }

        let file_path = file_path.ok_or_else(|| error::validation("file", "No file uploaded"))?;

        // 返回绝对路径
        let absolute_path = file_path.to_string_lossy().to_string();

        Ok(Response::new(UploadFileResponse {
            file_path: absolute_path,
            file_size: total_size,
        }))
    }

    #[require_permission("excel", "write")]
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
            .map_err(error::err_to_status)?;

        Ok(Response::new(ImportResultResponse {
            success_count: result.success_count as i32,
            failed_count: result.failed_count as i32,
            errors: result.errors,
        }))
    }

    #[require_permission("excel", "read")]
    async fn export_excel(&self, request: Request<ExportExcelRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.excel_service();

        let path = Path::new(&req.file_path);
        srv.export_products_to_excel(&state.pool(), path).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission("excel", "read")]
    async fn get_progress(&self, request: Request<Empty>) -> GrpcResult<ExcelProgressResponse> {
        let state = AppState::get().await;
        let srv = state.excel_service();

        let progress = srv.get_progress();

        Ok(Response::new(ExcelProgressResponse {
            current: progress.current as i32,
            total: progress.total as i32,
        }))
    }

    type DownloadExportFileStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission("excel", "read")]
    async fn download_export_file(
        &self,
        request: Request<DownloadExportFileRequest>,
    ) -> Result<Response<Self::DownloadExportFileStream>, tonic::Status> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.excel_service();

        let (bytes, file_name) = match req.export_type.as_str() {
            "products_without_price" => {
                let b = srv
                    .export_products_without_price_to_bytes(&state.pool())
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "products_without_price_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
            "boms_without_labor_cost" => {
                let b = srv
                    .export_boms_without_labor_cost_to_bytes(&state.pool())
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "boms_without_labor_cost_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
            _ => {
                // 默认导出所有产品
                let b = srv
                    .export_products_to_bytes(&state.pool())
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "products_export_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
        };

        let file_size = bytes.len() as i64;

        // 创建流式响应
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            // 发送元数据
            let metadata = FileMetadata {
                file_name,
                file_size,
                content_type: super::EXCEL_MIME_TYPE.to_string(),
            };
            let first_msg = DownloadFileResponse {
                data: Some(download_file_response::Data::Metadata(metadata)),
            };
            if tx.send(Ok(first_msg)).await.is_err() {
                return;
            }

            // 分块发送文件内容
            for chunk in bytes.chunks(super::STREAM_CHUNK_SIZE) {
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
