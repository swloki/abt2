pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{PaymentSchedule, PaymentScheduleInput};
pub use service::PaymentScheduleService;

pub fn new_payment_schedule_service(pool: sqlx::PgPool) -> impl PaymentScheduleService {
    implt::PaymentScheduleServiceImpl::new(pool)
}
