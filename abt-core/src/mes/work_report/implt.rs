use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::WorkReportRepo;
use super::service::WorkReportService;
use crate::mes::production_batch::repo::WorkOrderRoutingRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct WorkReportServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl WorkReportServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkReportService for WorkReportServiceImpl {
    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<WorkReport> {
        WorkReportRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkReport"))
    }

    async fn list_by_work_order(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>> {
        WorkReportRepo::list_by_work_order(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_by_batch(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<WorkReport>> {
        WorkReportRepo::list_by_batch(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReportListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReportListItem>> {
        WorkReportRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn calculate_wage(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        worker_id: i64,
        date_range: DateRange,
    ) -> Result<WageSummary> {
        let reports = WorkReportRepo::list_by_worker_and_date_range(
            &mut *db,
            worker_id,
            date_range.from,
            date_range.to,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let mut total_amount = Decimal::ZERO;
        let mut details = Vec::new();

        // 批量预加载所有相关工单的工序（N+1 修复）
        let wo_ids: Vec<i64> = {
            let mut ids: Vec<i64> = reports.iter().map(|r| r.work_order_id).collect();
            ids.sort();
            ids.dedup();
            ids
        };
        let all_routings = WorkOrderRoutingRepo::get_by_work_order_ids(&mut *db, &wo_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let routing_map: std::collections::HashMap<i64, _> =
            all_routings.iter().map(|r| (r.id, r)).collect();

        for report in &reports {
            let routing_info = routing_map.get(&report.routing_id);

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

    async fn list_all_wage_summaries(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        date_range: DateRange,
    ) -> Result<Vec<WageSummary>> {
        let all_reports = WorkReportRepo::list_by_date_range(
            &mut *db,
            date_range.from,
            date_range.to,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // Group by worker_id
        let mut worker_reports: std::collections::HashMap<i64, Vec<&WorkReport>> = std::collections::HashMap::new();
        for r in &all_reports {
            worker_reports.entry(r.worker_id).or_default().push(r);
        }

        let mut summaries = Vec::new();
        for (worker_id, reports) in worker_reports {
            let mut total_amount = Decimal::ZERO;
            let mut details = Vec::new();

            for report in reports {
                let routings = WorkOrderRoutingRepo::get_by_work_order_id(
                    &mut *db,
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

            summaries.push(WageSummary {
                worker_id,
                period_start: date_range.from,
                period_end: date_range.to,
                total_amount,
                details,
            });
        }

        // Sort by total_amount descending
        summaries.sort_by(|a, b| b.total_amount.cmp(&a.total_amount));
        Ok(summaries)
    }

    async fn get_detail_lookups(
        &self,
        db: PgExecutor<'_>,
        report: &WorkReport,
    ) -> Result<ReportDetailLookups> {
        let wo: Option<(String,)> = sqlx::query_as(
            "SELECT doc_number FROM work_orders WHERE id = $1",
        )
        .bind(report.work_order_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let batch: Option<(String,)> = sqlx::query_as(
            "SELECT batch_no FROM production_batches WHERE id = $1",
        )
        .bind(report.batch_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let routing: Option<(String,)> = sqlx::query_as(
            "SELECT process_name FROM work_order_routings WHERE id = $1",
        )
        .bind(report.routing_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let worker: Option<(String,)> = sqlx::query_as(
            "SELECT display_name FROM users WHERE user_id = $1",
        )
        .bind(report.worker_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        Ok(ReportDetailLookups {
            wo_doc_number: wo.map(|r| r.0),
            batch_no: batch.map(|r| r.0),
            process_name: routing.map(|r| r.0),
            worker_name: worker.map(|r| r.0),
        })
    }
}
