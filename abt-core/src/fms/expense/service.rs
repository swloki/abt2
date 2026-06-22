use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait ExpenseReimbursementService: Send + Sync {
    /// 创建报销单（草稿状态）
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateExpenseReq,
    ) -> Result<i64>;

    /// Draft → Submitted（提交审批，自动获取直属上级）
    async fn submit(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// Submitted → SupervisorApproved（直属上级审批）
    async fn supervisor_approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: SupervisorApproveReq,
    ) -> Result<()>;

    /// SupervisorApproved → FinanceApproved（财务审批）
    async fn finance_approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: FinanceApproveReq,
    ) -> Result<()>;

    /// FinanceApproved → Approved（总经理审批）
    async fn approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// Approved → Paid（出纳付款 + 付款信息留痕）
    async fn pay(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: PayReq,
    ) -> Result<()>;

    /// 取消报销单（任意非终态 → Cancelled）
    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 查询单个报销单
    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ExpenseReimbursement>;

    /// 分页查询报销单列表
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>>;

    /// 查询报销单明细列表
    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reimbursement_id: i64,
    ) -> Result<Vec<ExpenseReimbursementItem>>;

    /// 获取审批进度（从 state machine history 构建）
    async fn get_approval_progress(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Vec<ApprovalProgressNode>>;

    // ── 附件管理 ──

    /// 获取凭证附件列表（按 sort_order 排序）
    async fn list_attachments(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense_id: i64,
    ) -> Result<Vec<ExpenseAttachment>>;

    /// 上传凭证附件
    async fn upload_attachment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense_id: i64,
        req: CreateAttachmentReq,
    ) -> Result<i64>;

    /// 删除凭证附件
    async fn delete_attachment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        attachment_id: i64,
    ) -> Result<()>;

    // ── 仪表盘用 ──

    /// 待审报销统计: (count, total_amount)
    async fn pending_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<(i64, Decimal)>;
}
