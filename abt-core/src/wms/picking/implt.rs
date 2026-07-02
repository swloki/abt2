use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CreatePickingReq, DoneItemReq, PickingFilter, StockPicking, StockPickingItem};
use super::repo::PickingRepo;
use super::service::PickingService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};
use crate::wms::enums::PickingStatus;

pub struct PickingServiceImpl {
    // 阶段 2-5 done 实现时，用于按需获取 InventoryTransactionService / DocumentSequenceService 等共享服务
    #[allow(dead_code)]
    pool: PgPool,
}

impl PickingServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 生成单据号（阶段 1 兜底：type 前缀 + 时间戳）
    /// TODO（决策点 4）：接入 DocumentSequenceService，按 picking_type 分配连续序号
    fn generate_doc_number(req: &CreatePickingReq) -> String {
        format!(
            "{}{}",
            req.picking_type.doc_prefix(),
            chrono::Utc::now().format("%Y%m%d%H%M%S%.f")
        )
    }
}

#[async_trait]
impl PickingService for PickingServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePickingReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::Validation("作业单据至少需要一条明细".to_string()));
        }

        // 校验：调拨类型源/目标仓库不能相同（仅当两者都指定时）
        if let (Some(from), Some(to)) = (req.from_warehouse_id, req.to_warehouse_id)
            && from == to
        {
            return Err(DomainError::BusinessRule(
                "源仓库和目标仓库不能相同".to_string(),
            ));
        }

        let doc_number = Self::generate_doc_number(&req);

        let picking =
            PickingRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id).await?;

        Ok(picking.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<StockPicking> {
        PickingRepo::get_by_id(&mut *db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("作业单据"))
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
    ) -> Result<Vec<StockPickingItem>> {
        PickingRepo::get_items(&mut *db, picking_id).await
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PickingFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockPicking>> {
        PickingRepo::list(&mut *db, &filter, page, page_size).await
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Confirmed".to_string(),
            });
        }
        PickingRepo::update_status(&mut *db, id, PickingStatus::Confirmed).await?;
        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        match picking.status {
            PickingStatus::Draft | PickingStatus::Confirmed => {
                PickingRepo::update_status(&mut *db, id, PickingStatus::Cancelled).await?;
                Ok(())
            }
            other => Err(DomainError::InvalidStateTransition {
                from: format!("{other:?}"),
                to: "Cancelled".to_string(),
            }),
        }
    }

    async fn done(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        items: Vec<DoneItemReq>,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Done".to_string(),
            });
        }

        // 阶段 1：仅写回行级 qty_done + 状态转换。
        // TODO（阶段 2-5）：按 picking_type 分发——
        //   - 写 inventory_transactions 流水（IncomingPurchase→PurchaseReceipt / OutgoingSales→SalesShipment ...）
        //   - 回写来源单据（PO/WO/SO 的已收/已发量）
        //   - 发 PickingDone 事件（或保留各域事件，见决策点 5）
        for it in &items {
            PickingRepo::update_item_done(
                &mut *db,
                it.item_id,
                it.qty_done,
                it.batch_no.as_deref(),
                it.from_bin_id,
                it.to_bin_id,
            )
            .await?;
        }

        PickingRepo::set_done(&mut *db, id).await?;
        Ok(())
    }
}
