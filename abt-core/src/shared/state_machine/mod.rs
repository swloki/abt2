pub mod implt;
pub mod model;
pub mod service;

pub use model::{EntityStateLog, StateDefinition, StateDefinitionInput, StateTransitionDef, TransitionDefInput};
pub use service::StateMachineService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_state_machine_service(pool: PgPool) -> impl StateMachineService {
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(crate::shared::event_bus::implt::DomainEventBusImpl::new(pool.clone()));
    implt::StateMachineServiceImpl::new(pool, event_bus)
}
