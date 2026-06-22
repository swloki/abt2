use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait PurchaseInvoiceService: Send + Sync {
    /// 创建采购发票（Draft 状态）
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePurchaseInvoiceReq,
    ) -> Result<i64>;

    /// 过账采购发票（Draft → Posted）
    /// 生成 GL 凭证：借库存商品+进项税额，贷应付账款
    async fn post(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 创建红字采购发票
    async fn create_return(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        original_invoice_id: i64,
    ) -> Result<i64>;

    /// 取消采购发票（Posted → Cancelled）
    /// 同步取消对应的 GL 凭证
    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 获取采购发票详情
    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<(PurchaseInvoice, Vec<PurchaseInvoiceItem>)>;

    /// 列表查询
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PurchaseInvoiceFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseInvoice>>;
}
