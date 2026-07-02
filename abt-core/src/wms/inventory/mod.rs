pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::InventoryService;

pub fn new_inventory_service() -> impl InventoryService {
    implt::InventoryServiceImpl::new()
}