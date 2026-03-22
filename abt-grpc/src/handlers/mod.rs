//! gRPC Handlers

pub mod bom;
pub mod convert;
pub mod excel;
pub mod inventory;
pub mod labor_process;
pub mod location;
pub mod price;
pub mod product;
pub mod term;
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
};

pub type GrpcResult<T> = Result<tonic::Response<T>, tonic::Status>;
