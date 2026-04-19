//! gRPC Handlers

pub mod auth;
pub mod bom;
pub mod convert;
pub mod bom_category;
pub mod department;
pub mod excel;
pub mod inventory;
pub mod labor_process;
pub mod location;
pub mod permission;
pub mod price;
pub mod product;
pub mod role;
pub mod term;
pub mod user;
pub mod warehouse;

pub use crate::generated::abt::v1::{
    abt_bom_category_service_server::AbtBomCategoryServiceServer,
    abt_bom_service_server::AbtBomServiceServer,
    abt_excel_service_server::AbtExcelServiceServer,
    abt_inventory_service_server::AbtInventoryServiceServer,
    abt_labor_process_service_server::AbtLaborProcessServiceServer,
    abt_location_service_server::AbtLocationServiceServer,
    abt_price_service_server::AbtPriceServiceServer,
    abt_product_service_server::AbtProductServiceServer,
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
