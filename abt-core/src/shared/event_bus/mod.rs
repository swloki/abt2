pub mod dead_letter;
pub mod implt;
pub mod model;
pub mod processor;
pub mod registry;
pub(crate) mod repo;
pub mod service;

pub use model::{DomainEvent, EventPublishRequest, EventQuery};
pub use service::DomainEventBus;
pub use registry::{EventHandler, EventHandlerRegistry, EventHandlerRegistryImpl};
pub use dead_letter::{DeadLetterService, DeadLetterServiceImpl};
pub use processor::EventProcessor;

use sqlx::PgPool;

pub fn new_domain_event_bus(pool: PgPool) -> impl DomainEventBus {
    implt::DomainEventBusImpl::new(pool)
}
