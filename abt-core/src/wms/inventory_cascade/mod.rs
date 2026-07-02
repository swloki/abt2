pub mod model;
pub mod service;
pub(crate) mod repo;
pub mod implt;

pub use model::*;
pub use service::InventoryCascadeService;

pub fn new_inventory_cascade_service() -> impl InventoryCascadeService {
    implt::InventoryCascadeServiceImpl::new()
}