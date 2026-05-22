use async_trait::async_trait;

#[async_trait]
pub trait StateMachineService : Send + Sync {
    // TODO: define interface methods
}
