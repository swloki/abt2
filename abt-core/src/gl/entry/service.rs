use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait GlEntryService: Send + Sync {
    /// 创建手工凭证（Draft），source_type = GlEntry
    async fn create_manual(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateManualEntryReq,
    ) -> Result<i64>;

    /// 过账：Draft → Posted，校验借贷平衡 + 期间 open + 科目末级
    async fn post(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 作废：Posted → Cancelled，乐观锁，记审计
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 获取凭证详情（头 + 行）
    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<(GlEntry, Vec<GlEntryLine>)>;

    /// 列表查询（支持分页和过滤）
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: GlEntryFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<GlEntry>>;

    /// 试算平衡表（按期间，只统计 posted 凭证）
    async fn trial_balance(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period: String,
    ) -> Result<TrialBalance>;

    /// 总账明细账（按科目，包含对方科目和累计余额）
    async fn general_ledger(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        account_id: i64,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<GlDetailRow>>;

    /// 获取科目余额（含期初 + posted 分录，可选日期切片）
    async fn get_account_balance(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        account_id: i64,
        period: Option<String>,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Decimal>;

    /// 业务单据过账入口：一步建 posted 凭证（status=Posted），同事务
    /// 校验借贷平衡 + 期间 open + 科目末级；source_type/source_id 反查来源
    async fn post_from_source(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
        entry_date: NaiveDate,
        description: String,
        lines: Vec<GlEntryLineInput>,
    ) -> Result<i64>;
}
