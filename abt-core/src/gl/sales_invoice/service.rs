use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait SalesInvoiceService: Send + Sync {
    /// 创建销售发票（Draft 状态）
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateSalesInvoiceReq,
    ) -> Result<i64>;

    /// 过账销售发票（Draft → Posted）
    /// 生成 GL 凭证：借应收账款，贷主营业务收入+销项税额
    async fn post(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 取消销售发票（Posted → Cancelled）
    /// 同步取消对应的 GL 凭证
    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 获取销售发票详情
    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<(SalesInvoice, Vec<SalesInvoiceItem>)>;

    /// 列表查询
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: SalesInvoiceFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesInvoice>>;
}
