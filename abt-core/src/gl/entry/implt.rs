use sqlx::PgPool;
use rust_decimal::Decimal;
use std::collections::HashMap;
use chrono::NaiveDate;

use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result};

use super::model::*;
use super::repo::GlEntryRepo;
use super::service::GlEntryService;
use crate::gl::account::new_gl_account_service;
use crate::gl::account::service::GlAccountService;
use crate::gl::enums::EntryStatus;
use crate::gl::period::new_gl_period_service;
use crate::gl::period::service::GlPeriodService;

pub struct GlEntryServiceImpl {
    pool: PgPool,
}

impl GlEntryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GlEntryService for GlEntryServiceImpl {
    async fn create_manual(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateManualEntryReq,
    ) -> Result<i64> {
        // 至少两行
        if req.lines.len() < 2 {
            return Err(DomainError::validation("at least 2 lines required"));
        }

        // 校验每行金额
        for line in &req.lines {
            if line.debit < Decimal::ZERO || line.credit < Decimal::ZERO {
                return Err(DomainError::validation("negative amount"));
            }
            // 每行必须 借贷互斥（不能同时有金额，不能同时为0）
            let has_debit = line.debit > Decimal::ZERO;
            let has_credit = line.credit > Decimal::ZERO;
            if has_debit == has_credit {
                return Err(DomainError::validation(
                    "each line must be debit XOR credit",
                ));
            }

            // 校验科目存在且是末级
            let acct = new_gl_account_service(self.pool.clone())
                .get(ctx, db, line.account_id)
                .await?;
            if !acct.is_detail {
                return Err(DomainError::business_rule(
                    "Only detail accounts can be posted",
                ));
            }
        }

        // 期间必须 open（create 时即校验，避免录到已关期）
        let period = new_gl_period_service(self.pool.clone())
            .resolve_open(ctx, db, req.entry_date)
            .await?
            .name;

        // 生成凭证号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::GlEntry)
            .await?;

        // 插入凭证头
        let id = GlEntryRepo::create_entry(
            db,
            &doc_number,
            &req,
            &period,
            DocumentType::GlEntry,
            ctx.operator_id,
        )
        .await?;

        // 批量插入行
        GlEntryRepo::batch_lines(db, id, &req.lines).await?;

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "GlEntry",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn post(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        // 获取凭证头
        let entry = GlEntryRepo::get_entry(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("GlEntry"))?;

        // 只有 Draft 可以过账
        if entry.status != EntryStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft entries can be posted",
            ));
        }

        // 获取所有行
        let lines = GlEntryRepo::list_lines(db, id).await?;

        // 校验借贷平衡
        let total_debit: Decimal = lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = lines.iter().map(|l| l.credit).sum();
        if total_debit != total_credit {
            return Err(DomainError::business_rule("UnbalancedEntry"));
        }
        if total_debit == Decimal::ZERO {
            return Err(DomainError::business_rule("ZeroEntry"));
        }

        // 期间锁定复核（create 时已校验，post 时再次确认）
        new_gl_period_service(self.pool.clone())
            .resolve_open(ctx, db, entry.entry_date)
            .await?;

        // 更新状态（Draft → Posted）+ 乐观锁
        let rows = GlEntryRepo::update_status(
            db,
            id,
            EntryStatus::Posted,
            total_debit,
            total_credit,
            entry.version,
        )
        .await?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "GlEntry",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": "Draft",
                        "to": "Posted"
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        // 获取凭证头
        let entry = GlEntryRepo::get_entry(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("GlEntry"))?;

        // 只有 Posted 可以作废
        if entry.status != EntryStatus::Posted {
            return Err(DomainError::business_rule(
                "Only Posted entries can be cancelled",
            ));
        }

        // 更新状态（Posted → Cancelled）+ 乐观锁
        let rows = GlEntryRepo::update_status(
            db,
            id,
            EntryStatus::Cancelled,
            entry.total_debit,
            entry.total_credit,
            entry.version,
        )
        .await?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "GlEntry",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": "Posted",
                        "to": "Cancelled"
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<(GlEntry, Vec<GlEntryLine>)> {
        let entry = GlEntryRepo::get_entry(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("GlEntry"))?;
        let lines = GlEntryRepo::list_lines(db, id).await?;
        Ok((entry, lines))
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: GlEntryFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<GlEntry>> {
        let (items, total) =
            GlEntryRepo::query(db, &filter, &page, ctx.data_scope, ctx.operator_id, ctx.department_id)
                .await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn trial_balance(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period: String,
    ) -> Result<TrialBalance> {
        let mut rows = GlEntryRepo::sum_lines_by_period(db, &period).await?;

        // 计算每个科目的期末余额
        // 借方科目：余额 = 期初 + 借 - 贷
        // 贷方科目：余额 = 期初 + 贷 - 借
        for row in &mut rows {
            let account = new_gl_account_service(self.pool.clone())
                .get(ctx, db, row.account_id)
                .await?;

            // 使用 BalanceDirection 枚举判断
            match account.balance_direction {
                crate::gl::enums::BalanceDirection::Debit => {
                    // 借方科目
                    row.end_balance = account.opening_balance + row.period_debit - row.period_credit;
                }
                crate::gl::enums::BalanceDirection::Credit => {
                    // 贷方科目
                    row.end_balance = account.opening_balance + row.period_credit - row.period_debit;
                }
            }
        }

        // 汇总借贷
        let total_debit: Decimal = rows.iter().map(|r| r.period_debit).sum();
        let total_credit: Decimal = rows.iter().map(|r| r.period_credit).sum();

        Ok(TrialBalance {
            rows,
            total_debit,
            total_credit,
        })
    }

    async fn general_ledger(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        account_id: i64,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<GlDetailRow>> {
        // 获取该科目所有 posted 分录（按日期排序）
        let posted_lines =
            GlEntryRepo::get_posted_lines_by_account(db, account_id, from, to).await?;

        // 获取科目信息（用于计算余额方向）
        let account = new_gl_account_service(self.pool.clone())
            .get(ctx, db, account_id)
            .await?;

        // 按凭证分组，查找对方科目
        let mut entry_map: HashMap<i64, Vec<&GlEntryLine>> = HashMap::new();
        let mut entry_details: HashMap<i64, (&GlEntry, &GlEntryLine)> = HashMap::new();

        for (entry, line) in &posted_lines {
            entry_map
                .entry(entry.id)
                .or_insert_with(Vec::new)
                .push(line);
            entry_details.insert(entry.id, (entry, line));
        }

        // 计算对方科目：取同凭证其他行的第一个 account_id
        let mut counterpart_map: HashMap<i64, Option<i64>> = HashMap::new();
        for (&entry_id, lines) in &entry_map {
            if lines.len() > 1 {
                // 有其他行，取第一个非当前行的 account_id
                let current_account_id = lines[0].account_id;
                for line in lines {
                    if line.account_id != current_account_id {
                        counterpart_map.insert(entry_id, Some(line.account_id));
                        break;
                    }
                }
                if counterpart_map.get(&entry_id).is_none() {
                    counterpart_map.insert(entry_id, None);
                }
            } else {
                // 单行凭证，无对方科目
                counterpart_map.insert(entry_id, None);
            }
        }

        // 按日期排序，计算累计余额
        let mut result = Vec::new();
        let mut running_balance = account.opening_balance;

        // 按 entry_date 和 id 排序（确保顺序稳定）
        let mut sorted_entries: Vec<_> = posted_lines.iter().collect();
        sorted_entries.sort_by_key(|(e, _)| (e.entry_date, e.id));

        for (entry, line) in sorted_entries {
            let counterpart = counterpart_map.get(&entry.id).copied().flatten();

            // 累计余额：借方科目 +debit-credit，贷方科目 +credit-debit
            match account.balance_direction {
                crate::gl::enums::BalanceDirection::Debit => {
                    running_balance = running_balance + line.debit - line.credit;
                }
                crate::gl::enums::BalanceDirection::Credit => {
                    running_balance = running_balance + line.credit - line.debit;
                }
            }

            result.push(GlDetailRow {
                entry_id: entry.id,
                doc_number: entry.doc_number.clone(),
                entry_date: entry.entry_date,
                memo: entry.description.clone(),
                counterpart_account_id: counterpart,
                debit: line.debit,
                credit: line.credit,
                running_balance,
            });
        }

        Ok(result)
    }

    async fn get_account_balance(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        account_id: i64,
        period: Option<String>,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Decimal> {
        // 获取科目信息
        let account = new_gl_account_service(self.pool.clone())
            .get(ctx, db, account_id)
            .await?;

        // 获取 posted 分录汇总（按期间和日期切片）
        let (total_debit, total_credit) = GlEntryRepo::get_account_posted_summary(
            db,
            account_id,
            period.as_deref(),
            as_of_date,
        )
        .await?;

        // 余额 = 期初 + 分录净额
        // 借方科目：期初 + 借 - 贷
        // 贷方科目：期初 + 贷 - 借
        let balance = match account.balance_direction {
            crate::gl::enums::BalanceDirection::Debit => {
                account.opening_balance + total_debit - total_credit
            }
            crate::gl::enums::BalanceDirection::Credit => {
                account.opening_balance + total_credit - total_debit
            }
        };

        Ok(balance)
    }
}
