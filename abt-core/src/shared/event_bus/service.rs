use async_trait::async_trait;

#[async_trait]
pub trait DomainEventBus : Send + Sync {
    // TODO: define interface methods
}
