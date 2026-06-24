pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{InventoryReservation, ReservationDetail, ReserveRequest};
pub use service::InventoryReservationService;

use sqlx::PgPool;

pub fn new_inventory_reservation_service(pool: PgPool) -> impl InventoryReservationService {
    implt::InventoryReservationServiceImpl::new(pool)
}
