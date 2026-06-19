use sqlx::PgPool;
use chrono::{Utc, DateTime};

use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PgExecutor, Result};

use super::model::*;
use super::repo::GlPeriodRepo;
use super::service::GlPeriodService;

pub struct GlPeriodServiceImpl {
    pool: PgPool,
}

impl GlPeriodServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GlPeriodService for GlPeriodServiceImpl {
    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PeriodFilter,
    ) -> Result<Vec<AccountingPeriod>> {
        GlPeriodRepo::list(db, &filter).await
    }

    async fn resolve_open(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        entry_date: chrono::NaiveDate,
    ) -> Result<AccountingPeriod> {
        // 查找期间
        let period = GlPeriodRepo::get_by_date(db, entry_date)
            .await?
            .ok_or_else(|| DomainError::not_found("AccountingPeriod"))?;

        // 检查期间是否为 Open 状态
        if period.status != super::super::enums::PeriodStatus::Open {
            return Err(DomainError::business_rule("PeriodClosed"));
        }

        Ok(period)
    }

    async fn close(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period_id: i64,
    ) -> Result<()> {
        // 1. 校验该期是否存在 draft 凭证
        let draft_count: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"
                SELECT COUNT(*)
                FROM gl_entries
                WHERE period = (SELECT name FROM accounting_periods WHERE id = $1)
                  AND status = 1  -- Draft
                  AND deleted_at IS NULL
            "#
        )
        .bind(period_id)
        .fetch_one(&mut *db)
        .await?;

        if draft_count > 0 {
            return Err(DomainError::business_rule("HasDraftEntries"));
        }

        // 2. 获取当前期间（用于乐观锁）
        let period = sqlx::query_as::<sqlx::Postgres, AccountingPeriod>(
            "SELECT id, name, start_date, end_date, status, fiscal_year, closed_at, closed_by, version, created_at, updated_at FROM accounting_periods WHERE id = $1"
        )
        .bind(period_id)
        .fetch_optional(&mut *db)
        .await?
        .ok_or_else(|| DomainError::not_found("AccountingPeriod"))?;

        // 3. 乐观锁更新状态（Open → Closed）
        use super::super::enums::PeriodStatus;
        let now: DateTime<Utc> = Utc::now();
        let rows = GlPeriodRepo::update_status(
            db,
            period_id,
            PeriodStatus::Closed,
            period.version,
            Some(now),
            Some(ctx.operator_id),
        )
        .await?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 4. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "AccountingPeriod",
                    entity_id: period_id,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "status": "Open -> Closed",
                        "closed_at": now,
                        "closed_by": ctx.operator_id,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }
}
