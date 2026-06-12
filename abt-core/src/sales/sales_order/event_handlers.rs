//! 销售 — DemandConfirmed / DemandRejected 事件处理器
//!
//! 包装 implt.rs 中已有的 handle_demand_confirmed / handle_demand_rejected 函数，
//! 使其符合 EventHandler trait，可被 EventProcessor 注册和调度。

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{Result, ServiceContext};

/// DemandConfirmed 事件处理器 — 异步更新履行计划行和订单行状态
///
/// 事务边界：独立事务（与 confirm 事务分离），避免跨聚合死锁。
/// 幂等保证：Handler 内部使用前置状态校验的单条 UPDATE（见 implt.rs）。
pub struct SalesDemandConfirmedHandler {
    pool: PgPool,
}

impl SalesDemandConfirmedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for SalesDemandConfirmedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system();

        // 复用 implt.rs 中已有的逻辑
        super::implt::handle_demand_confirmed(
            self.pool.clone(),
            &ctx,
            &mut conn,
            event,
        ).await
    }

    fn name(&self) -> &str {
        "sales_demand_confirmed"
    }
}

/// DemandRejected 事件处理器 — 回退履行计划行和订单行到 Pending
pub struct SalesDemandRejectedHandler {
    pool: PgPool,
}

impl SalesDemandRejectedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for SalesDemandRejectedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system();

        super::implt::handle_demand_rejected(
            self.pool.clone(),
            &ctx,
            &mut conn,
            event,
        ).await
    }

    fn name(&self) -> &str {
        "sales_demand_rejected"
    }
}
