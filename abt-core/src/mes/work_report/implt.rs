use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::WorkReportRepo;
use super::service::WorkReportService;
use crate::mes::production_batch::repo::WorkOrderRoutingRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct WorkReportServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl WorkReportServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkReportService for WorkReportServiceImpl {
    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<WorkReport> {
        WorkReportRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkReport"))
    }

    async fn list_by_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>> {
        WorkReportRepo::list_by_work_order(&mut *ctx.executor, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_by_batch(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
    ) -> Result<Vec<WorkReport>> {
        WorkReportRepo::list_by_batch(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn calculate_wage(
        &self,
        ctx: ServiceContext<'_>,
        worker_id: i64,
        date_range: DateRange,
    ) -> Result<WageSummary> {
        let reports = WorkReportRepo::list_by_worker_and_date_range(
            &mut *ctx.executor,
            worker_id,
            date_range.from,
            date_range.to,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let mut total_amount = Decimal::ZERO;
        let mut details = Vec::new();

        for report in &reports {
            // 查找工序获取 unit_price 和 process_name
            let routings = WorkOrderRoutingRepo::get_by_work_order_id(
                &mut *ctx.executor,
                report.work_order_id,
            )
            .await
            .ok()
            .unwrap_or_default();

            let routing_info = routings.into_iter().find(|r| r.id == report.routing_id);

            let (process_name, unit_price) = routing_info
                .as_ref()
                .map(|r| (r.process_name.clone(), r.unit_price.unwrap_or(Decimal::ZERO)))
                .unwrap_or_else(|| (String::new(), Decimal::ZERO));

            // 工资公式：(completed_qty + non_operator_defect_qty) * unit_price
            let non_operator_defect_qty = match report.defect_reason {
                Some(reason) if reason.affect_wage() => report.defect_qty,
                _ => Decimal::ZERO,
            };
            let wage_amount = (report.completed_qty + non_operator_defect_qty) * unit_price;
            total_amount += wage_amount;

            details.push(WageDetail {
                work_order_id: report.work_order_id,
                batch_id: report.batch_id,
                routing_id: report.routing_id,
                process_name,
                report_date: report.report_date,
                completed_qty: report.completed_qty,
                defect_qty: report.defect_qty,
                defect_reason: report.defect_reason,
                unit_price,
                wage_amount,
            });
        }

        Ok(WageSummary {
            worker_id,
            period_start: date_range.from,
            period_end: date_range.to,
            total_amount,
            details,
        })
    }
}
