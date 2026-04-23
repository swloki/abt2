//! gRPC Handlers

pub mod auth;
pub mod bom;
pub mod convert;
pub mod bom_category;
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

pub use crate::generated::abt::v1::{
    abt_bom_category_service_server::AbtBomCategoryServiceServer,
    abt_bom_service_server::AbtBomServiceServer,
    abt_excel_service_server::AbtExcelServiceServer,
    abt_inventory_service_server::AbtInventoryServiceServer,
    abt_labor_process_service_server::AbtLaborProcessServiceServer,
    abt_labor_process_dict_service_server::AbtLaborProcessDictServiceServer,
    abt_location_service_server::AbtLocationServiceServer,
    abt_price_service_server::AbtPriceServiceServer,
    abt_product_service_server::AbtProductServiceServer,
    abt_routing_service_server::AbtRoutingServiceServer,
    abt_term_service_server::AbtTermServiceServer,
    abt_warehouse_service_server::AbtWarehouseServiceServer,
    auth_service_server::AuthServiceServer,
    department_service_server::DepartmentServiceServer,
    permission_service_server::PermissionServiceServer,
    role_service_server::RoleServiceServer,
    user_service_server::UserServiceServer,
};

pub type GrpcResult<T> = Result<tonic::Response<T>, tonic::Status>;

/// Convert an empty string to None, non-empty to Some.
pub fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
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
