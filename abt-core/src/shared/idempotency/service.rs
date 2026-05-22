use async_trait::async_trait;

#[async_trait]
pub trait IdempotencyService : Send + Sync {
    // TODO: define interface methods
}
