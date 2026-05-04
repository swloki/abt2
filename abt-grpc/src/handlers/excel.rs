//! Excel gRPC Handler

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use common::error;
use tonic::{Request, Response, Streaming};
use crate::generated::abt::v1::{abt_excel_service_server::AbtExcelService as GrpcExcelService, *};
use crate::handlers::{validate_upload_path, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use abt::{ExcelImportService, ExcelExportService, ImportSource, ExportRequest};

const IMPORT_KEY_PRODUCT_INVENTORY: &str = "product_inventory";
const IMPORT_KEY_WAREHOUSE_LOCATION: &str = "warehouse_location";
const EXPORT_TYPE_PRODUCTS_WITHOUT_PRICE: &str = "products_without_price";
const EXPORT_TYPE_WAREHOUSE_LOCATION: &str = "warehouse_location";

pub struct ExcelHandler {
    active_imports: Mutex<HashMap<String, Arc<abt::excel::ProgressTracker>>>,
}

const MAX_FILE_SIZE: i64 = 50 * 1024 * 1024; // 50MB

impl ExcelHandler {
    pub fn new() -> Self {
        Self {
            active_imports: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for ExcelHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcExcelService for ExcelHandler {
    #[require_permission(Resource::Excel, Action::Write)]
    async fn upload_file(
        &self,
        request: Request<Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, tonic::Status> {
        let upload_dir = std::env::temp_dir();

        tokio::fs::create_dir_all(&upload_dir)
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
                    let unique_name = format!("{}_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), file_name);
                    let path = upload_dir.join(&unique_name);
                    file_path = Some(path.clone());

                    file = Some(tokio::fs::File::create(&path)
                        .await
                        .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to create file: {}", e)))?);
                }
                Some(upload_file_request::Data::Chunk(chunk)) => {
                    if let Some(ref mut f) = file {
                        total_size += chunk.len() as i64;
                        if total_size > MAX_FILE_SIZE {
                            return Err(tonic::Status::resource_exhausted(
                                format!("文件大小超过限制 ({} bytes)", MAX_FILE_SIZE),
                            ));
                        }
                        use tokio::io::AsyncWriteExt;
                        f.write_all(&chunk)
                            .await
                            .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to write chunk: {}", e)))?;
                    } else {
                        return Err(tonic::Status::failed_precondition("File name must be sent first"));
                    }
                }
                None => {}
            }
        }

        if let Some(ref mut f) = file {
            use tokio::io::AsyncWriteExt;
            f.flush()
                .await
                .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to flush file: {}", e)))?;
        }

        let file_path = file_path.ok_or_else(|| error::validation("file", "No file uploaded"))?;

        let absolute_path = file_path.to_string_lossy().to_string();

        Ok(Response::new(UploadFileResponse {
            file_path: absolute_path,
            file_size: total_size,
        }))
    }

    #[require_permission(Resource::Excel, Action::Write)]
    async fn import_excel(
        &self,
        request: Request<ImportExcelRequest>,
    ) -> GrpcResult<ImportResultResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;

        let path = std::path::Path::new(&req.file_path);
        validate_upload_path(&req.file_path)?;

        // 读取文件内容到内存，使用 ImportSource::Bytes 导入
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法读取上传文件: {}", e)))?;

        let pool = state.pool();
        let source = ImportSource::Bytes(bytes);

        let result = match req.import_type.as_str() {
            "warehouse_location" => {
                let sync_mode = req.sync_mode.unwrap_or(false);
                let import_key = IMPORT_KEY_WAREHOUSE_LOCATION.to_string();
                let tracker = abt::excel::ProgressTracker::new();
                {
                    let mut guard = self.active_imports.lock().unwrap_or_else(|e| e.into_inner());
                    guard.insert(import_key.clone(), tracker);
                }
                let result = abt::excel::import_warehouse_locations(&pool, source, sync_mode).await;
                {
                    let mut guard = self.active_imports.lock().unwrap_or_else(|e| e.into_inner());
                    guard.remove(&import_key);
                }
                result.map_err(error::err_to_status)?
            }
            _ => {
                // 默认: 产品库存导入
                let tracker = abt::excel::ProgressTracker::new();
                let mut importer = abt::excel::ProductInventoryImporter::new(pool.clone(), tracker.clone());
                if let Some(operator_id) = req.operator_id {
                    importer = importer.with_operator(operator_id);
                }

                {
                    let mut guard = self.active_imports.lock().unwrap_or_else(|e| e.into_inner());
                    guard.insert(IMPORT_KEY_PRODUCT_INVENTORY.to_string(), tracker);
                }

                let result = importer.import(source).await;

                {
                    let mut guard = self.active_imports.lock().unwrap_or_else(|e| e.into_inner());
                    guard.remove(IMPORT_KEY_PRODUCT_INVENTORY);
                }

                result.map_err(error::err_to_status)?
            }
        };

        Ok(Response::new(ImportResultResponse {
            success_count: result.success_count as i32,
            failed_count: result.failed_count as i32,
            errors: result.errors,
            row_errors: result.row_errors.into_iter().map(|re| RowError {
                row_index: re.row_index as u32,
                column_name: re.column_name,
                reason: re.reason,
                raw_value: re.raw_value,
            }).collect(),
        }))
    }

    #[require_permission(Resource::Excel, Action::Read)]
    async fn export_excel(&self, request: Request<ExportExcelRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;

        validate_upload_path(&req.file_path)?;

        let exporter = abt::excel::ProductAllExporter::new(state.pool());
        let bytes = exporter.export(ExportRequest { params: () })
            .await
            .map_err(error::err_to_status)?;

        tokio::fs::write(&req.file_path, bytes)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法写入导出文件: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Excel, Action::Read)]
    async fn get_progress(&self, _request: Request<Empty>) -> GrpcResult<ExcelProgressResponse> {
        let guard = self.active_imports.lock().unwrap_or_else(|e| e.into_inner());
        let progress = guard
            .get(IMPORT_KEY_PRODUCT_INVENTORY)
            .map(|t| t.snapshot())
            .unwrap_or_default();

        Ok(Response::new(ExcelProgressResponse {
            current: progress.current as i32,
            total: progress.total as i32,
        }))
    }

    type DownloadExportFileStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::Excel, Action::Read)]
    async fn download_export_file(
        &self,
        request: Request<DownloadExportFileRequest>,
    ) -> Result<Response<Self::DownloadExportFileStream>, tonic::Status> {
        let req = request.into_inner();
        let state = AppState::get().await;

        let (bytes, file_name) = match req.export_type.as_str() {
            EXPORT_TYPE_WAREHOUSE_LOCATION => {
                let b = abt::excel::export_warehouse_locations_to_bytes(&state.pool())
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "warehouse_location_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
            EXPORT_TYPE_PRODUCTS_WITHOUT_PRICE => {
                let exporter = abt::excel::ProductWithoutPriceExporter::new(state.pool());
                let b = exporter.export(ExportRequest { params: () })
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "products_without_price_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
            _ => {
                let exporter = abt::excel::ProductAllExporter::new(state.pool());
                let b = exporter.export(ExportRequest { params: () })
                    .await
                    .map_err(error::err_to_status)?;
                let name = format!(
                    "products_export_{}.xlsx",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                );
                (b, name)
            }
        };

        Ok(Response::new(crate::handlers::stream_excel_bytes(file_name, bytes)))
    }
}
