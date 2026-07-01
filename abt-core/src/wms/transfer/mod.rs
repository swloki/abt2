pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{CreateTransferItemReq, CreateTransferReq, InventoryTransfer, TransferFilter, TransferItem};
pub use service::TransferService;

use sqlx::PgPool;

pub fn new_transfer_service(pool: PgPool) -> impl TransferService {
    implt::TransferServiceImpl::new(pool)
}
