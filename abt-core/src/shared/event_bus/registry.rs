use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use super::model::DomainEvent;
use crate::shared::enums::event::DomainEventType;
use crate::shared::types::error::DomainError;

/// 事件处理器 trait — 业务模块实现此接口
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &DomainEvent) -> Result<(), DomainError>;
    fn name(&self) -> &str;
}

/// 事件处理器注册表 trait
#[async_trait]
pub trait EventHandlerRegistry: Send + Sync {
    fn register(&self, event_type: DomainEventType, handler: Arc<dyn EventHandler>);
    async fn dispatch(&self, event: &DomainEvent) -> Result<(), DomainError>;
}

/// 基于 HashMap 的注册表实现
pub struct EventHandlerRegistryImpl {
    handlers: RwLock<HashMap<DomainEventType, Vec<Arc<dyn EventHandler>>>>,
}

impl EventHandlerRegistryImpl {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for EventHandlerRegistryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventHandlerRegistry for EventHandlerRegistryImpl {
    fn register(&self, event_type: DomainEventType, handler: Arc<dyn EventHandler>) {
        let mut map = self.handlers.write().expect("EventHandlerRegistry lock poisoned");
        map.entry(event_type).or_default().push(handler);
    }

    async fn dispatch(&self, event: &DomainEvent) -> Result<(), DomainError> {
        // 先 clone handler 列表，避免在锁内 await
        let handlers: Vec<Arc<dyn EventHandler>> = {
            let map = self.handlers.read().expect("EventHandlerRegistry lock poisoned");
            map.get(&event.event_type).cloned().unwrap_or_default()
        };

        // 顺序调用所有 handler
        for handler in handlers {
            handler.handle(event).await?;
        }

        Ok(())
    }
}
