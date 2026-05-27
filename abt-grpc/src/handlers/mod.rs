//! gRPC Handlers

pub mod auth;
pub mod bom;
pub mod convert;
pub mod bom_category;
pub mod category;
pub mod customer;
pub mod department;
pub mod excel;
pub mod inventory;
pub mod labor_process;
pub mod labor_process_dict;
pub mod location;
pub mod permission;
pub mod price;
pub mod product;
pub mod role;
pub mod routing;
pub mod term;
pub mod user;
pub mod warehouse;
pub mod notification;
pub mod sync_handler;
pub mod quotation;
pub mod sales_order;
pub mod sales_return;
pub mod shipping_request;
pub mod workflow;

pub use crate::generated::abt::v1::{
    abt_bom_category_service_server::AbtBomCategoryServiceServer,
    abt_category_service_server::AbtCategoryServiceServer,
    customer_service_server::CustomerServiceServer,
    abt_bom_service_server::AbtBomServiceServer,
    abt_excel_service_server::AbtExcelServiceServer,
    abt_inventory_service_server::AbtInventoryServiceServer,
    abt_labor_process_service_server::AbtLaborProcessServiceServer,
    abt_labor_process_dict_service_server::AbtLaborProcessDictServiceServer,
    abt_location_service_server::AbtLocationServiceServer,
    abt_price_service_server::AbtPriceServiceServer,
    abt_product_service_server::AbtProductServiceServer,
    abt_routing_service_server::AbtRoutingServiceServer,
    abt_notification_service_server::AbtNotificationServiceServer,
    abt_sync_service_server::AbtSyncServiceServer,
    abt_workflow_service_server::AbtWorkflowServiceServer,
    quotation_service_server::QuotationServiceServer,
    sales_order_service_server::SalesOrderServiceServer,
    sales_return_service_server::SalesReturnServiceServer,
    shipping_request_service_server::ShippingRequestServiceServer,
    abt_term_service_server::AbtTermServiceServer,
    abt_warehouse_service_server::AbtWarehouseServiceServer,
    auth_service_server::AuthServiceServer,
    department_service_server::DepartmentServiceServer,
    permission_service_server::PermissionServiceServer,
    role_service_server::RoleServiceServer,
    user_service_server::UserServiceServer,
};

pub type GrpcResult<T> = Result<tonic::Response<T>, tonic::Status>;

/// DomainError → tonic::Status 映射，所有迁移到 abt-core 的 handler 共用
pub fn domain_to_status(e: abt_core::shared::types::DomainError) -> tonic::Status {
    use abt_core::shared::types::DomainError;
    match e {
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::Duplicate(msg) => tonic::Status::already_exists(msg),
        DomainError::PermissionDenied(msg) => tonic::Status::permission_denied(msg),
        DomainError::BusinessRule(msg) => tonic::Status::failed_precondition(msg),
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::ConcurrentConflict => tonic::Status::aborted("Concurrent conflict"),
        DomainError::InvalidStateTransition { from, to } => {
            tonic::Status::failed_precondition(format!("Invalid state transition: {from} -> {to}"))
        }
        DomainError::Internal(e) => tonic::Status::internal(e.to_string()),
    }
}

/// Convert an empty string to None, non-empty to Some.
pub fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Convert an Option<DateTime> to RFC3339 string, empty if None.
pub fn dt_to_string(dt: Option<chrono::DateTime<chrono::Utc>>) -> String {
    dt.map(|d| d.to_rfc3339()).unwrap_or_default()
}

/// Convert an Option<serde_json::Value> to JSON string, empty if None.
pub fn json_to_string(v: Option<serde_json::Value>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

/// 验证文件路径在上传目录内，防止路径遍历
pub fn validate_upload_path(file_path: &str) -> Result<std::path::PathBuf, tonic::Status> {
    let upload_dir = std::env::temp_dir().canonicalize()
        .map_err(|e| crate::error::err_to_status(anyhow::anyhow!("无法解析上传目录: {}", e)))?;
    let canonical = std::path::Path::new(file_path).canonicalize()
        .map_err(|e| crate::error::err_to_status(anyhow::anyhow!("无法解析文件路径: {}", e)))?;
    if !canonical.starts_with(&upload_dir) {
        return Err(crate::error::validation("file_path", "只允许操作上传目录中的文件"));
    }
    Ok(canonical)
}

// ============================================================================
// 流式下载共享常量和工具函数
// ============================================================================

/// Excel 文件的 MIME 类型
pub const EXCEL_MIME_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// 流式传输的块大小 (64KB)
pub const STREAM_CHUNK_SIZE: usize = 64 * 1024;

/// 将字节流打包为 gRPC 流式下载响应
///
/// 先发送文件元数据，再分块发送文件内容。
/// 所有 handler 中的流式下载都应使用此函数，避免重复代码。
pub fn stream_excel_bytes(
    file_name: impl Into<String>,
    bytes: Vec<u8>,
) -> tokio_stream::wrappers::ReceiverStream<Result<crate::generated::abt::v1::DownloadFileResponse, tonic::Status>> {
    use crate::generated::abt::v1::{
        DownloadFileResponse, FileMetadata,
        download_file_response::Data,
    };

    let file_size = bytes.len() as i64;
    let file_name = file_name.into();
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    tokio::spawn(async move {
        let metadata = FileMetadata {
            file_name,
            file_size,
            content_type: EXCEL_MIME_TYPE.to_string(),
        };
        if tx
            .send(Ok(DownloadFileResponse {
                data: Some(Data::Metadata(metadata)),
            }))
            .await
            .is_err()
        {
            return;
        }

        for chunk in bytes.chunks(STREAM_CHUNK_SIZE) {
            if tx
                .send(Ok(DownloadFileResponse {
                    data: Some(Data::Chunk(chunk.to_vec())),
                }))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    tokio_stream::wrappers::ReceiverStream::new(rx)
}
