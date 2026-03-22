//! gRPC Handlers

pub mod bom;
pub mod convert;
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
    abt_bom_service_server::AbtBomServiceServer,
    abt_excel_service_server::AbtExcelServiceServer,
    abt_inventory_service_server::AbtInventoryServiceServer,
    abt_location_service_server::AbtLocationServiceServer,
    abt_price_service_server::AbtPriceServiceServer,
    abt_product_service_server::AbtProductServiceServer,
    abt_term_service_server::AbtTermServiceServer,
    abt_warehouse_service_server::AbtWarehouseServiceServer,
    permission_service_server::PermissionServiceServer,
    role_service_server::RoleServiceServer,
    user_service_server::UserServiceServer,
};

pub type GrpcResult<T> = Result<tonic::Response<T>, tonic::Status>;
