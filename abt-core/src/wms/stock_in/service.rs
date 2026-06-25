use crate::shared::types::{PgExecutor, Result, ServiceContext};

use super::model::ReceiveAndStockInReq;

/// 采购订单直收入库服务（取消来料通知后的采购入库闭环）。
///
/// 取消来料通知前，采购入库走来料通知 create→receive→inspect→ArrivalInspected 事件→
/// ArrivalAcceptedHandler 回写 PO received_qty/状态 + 立应付 + 成本。本 service 把整条闭环
/// 收敛成同步事务内编排（消除事件未处理窗口期断链），PO 直接收货即入库即立账。
#[async_trait::async_trait]
pub trait PurchaseStockInService: Send + Sync {
    /// 采购订单直收入库闭环（事务内 8 步）：
    /// 1. 幂等 try_claim → 2. 超收校验 → 3. record 库存(source=purchase_order) →
    /// 4. 增量累加 PO received_qty → 5. PO 状态流转 → 6. 立应付(PO 维度 upsert) →
    /// 7. 成本分录 → 8. 审计日志
    ///
    /// **不自开事务**：接收 `db: PgExecutor`，由调用方（abt-web handler）开 `state.pool.begin()`
    /// 后把 `&mut tx` 传入（范本 `wms_stock_in_create.rs::create_stock_in`）。
    async fn receive_and_stock_in(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReceiveAndStockInReq,
    ) -> Result<()>;
}
