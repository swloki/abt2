pub mod implt;
pub mod model;
pub mod service;

pub use model::{EntityStateLog, StateDefinition, StateDefinitionInput, StateTransitionDef, TransitionDefInput};
pub use service::StateMachineService;

use sqlx::PgPool;

pub fn new_state_machine_service(pool: PgPool) -> impl StateMachineService {
    implt::StateMachineServiceImpl::new(pool)
}
