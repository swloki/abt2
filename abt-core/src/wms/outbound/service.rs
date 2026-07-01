use std::collections::HashMap;

use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};
use crate::wms::inventory_transaction::model::InventoryTransaction;

#[async_trait]
pub trait ShippingRequestService: Send + Sync {
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateFromOrderReq,
    ) -> Result<i64>;

    /// 一键申请发货（订单详情页弹窗提交）：各行 warehouse=None（销售不指定仓库），
    /// 发货单跳过 Draft → 直接 Confirmed（入 work-center 待发货队列），回写订单 ShippingRequested。
    async fn request_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
        items: Vec<RequestShippingItemReq>,
    ) -> Result<i64>;

    async fn save_draft(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateDraftReq,
    ) -> Result<i64>;

    async fn update_draft(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateDraftReq,
    ) -> Result<()>;

    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingRequest>;

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateShippingReq,
    ) -> Result<()>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn pick(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn ship(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 直接发货（跳过拣货，未拣 Confirmed 单用）。仓库/库位由调用方传入（选仓 drawer）。
    /// 与 ship()（Picking→Shipped，仓库从 pick_list 取）共用 do_ship 核心逻辑。
    async fn direct_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        bin_id: Option<i64>,
    ) -> Result<()>;

    /// 发货核心（内部共享逻辑，外部请调 direct_ship / ship）。
    async fn do_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        existing: &ShippingRequest,
        shipping_items: &[ShippingRequestItem],
        order_id: i64,
        wh_bin: &HashMap<i64, (Option<i64>, Option<i64>)>,
        from_label: &str,
    ) -> Result<()>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        shipping_request_id: i64,
    ) -> Result<Vec<ShippingRequestItem>>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ShippingQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<ShippingRequest>>;

    /// Hub 摘要带数据（首屏轻量查询，含缺货 ATP 判定）
    async fn hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingHubSummary>;

    /// 本单相关的库存事务流水（disclosure 懒加载）
    async fn list_transactions(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<InventoryTransaction>>;
}
