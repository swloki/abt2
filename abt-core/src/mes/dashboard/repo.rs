use super::model::*;
use crate::shared::types::Result;

pub struct DashboardRepo;

impl DashboardRepo {
    pub async fn get_stats(executor: &mut sqlx::postgres::PgConnection) -> Result<DashboardStats> {
        let stats = sqlx::query_as::<_, DashboardStats>(
            "SELECT \
             (SELECT COUNT(*) FROM production_plans) AS plan_count, \
             (SELECT COUNT(*) FROM work_orders WHERE status IN (2,3)) AS active_order_count, \
             (SELECT COUNT(*) FROM production_batches WHERE status IN (1,2,3,4)) AS active_batch_count, \
             (SELECT COUNT(*) FROM production_receipts WHERE status = 1) AS pending_receipt_count, \
             (SELECT COALESCE(SUM(r.received_qty),0) FROM production_receipts r \
                WHERE r.status = 2 AND EXTRACT(MONTH FROM r.created_at) = EXTRACT(MONTH FROM CURRENT_DATE) \
                AND EXTRACT(YEAR FROM r.created_at) = EXTRACT(YEAR FROM CURRENT_DATE)) AS completed_qty"
        ).fetch_one(&mut *executor).await?;
        Ok(stats)
    }

    pub async fn get_quick_entry_stats(executor: &mut sqlx::postgres::PgConnection) -> Result<QuickEntryStats> {
        let plan_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_plans")
            .fetch_one(&mut *executor).await?;
        let order_active: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM work_orders WHERE status IN (2,3)")
            .fetch_one(&mut *executor).await?;
        let batch_active: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_batches WHERE status IN (1,2,3,4)")
            .fetch_one(&mut *executor).await?;
        let report_month: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM work_reports WHERE EXTRACT(MONTH FROM created_at) = EXTRACT(MONTH FROM CURRENT_DATE) AND EXTRACT(YEAR FROM created_at) = EXTRACT(YEAR FROM CURRENT_DATE)")
            .fetch_one(&mut *executor).await?;
        let insp_pending: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_inspections WHERE result = 0")
            .fetch_one(&mut *executor).await?;
        let receipt_pending: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_receipts WHERE status = 1")
            .fetch_one(&mut *executor).await?;
        let batch_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&mut *executor).await?;
        let insp_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_inspections WHERE result = 0")
            .fetch_one(&mut *executor).await?;

        Ok(QuickEntryStats {
            plan_total: plan_total.0,
            order_active: order_active.0,
            batch_active: batch_active.0,
            report_month: report_month.0,
            insp_pending: insp_pending.0,
            receipt_pending: receipt_pending.0,
            batch_total: batch_total.0,
            insp_total: insp_total.0,
        })
    }

    pub async fn get_recent_ops(
        executor: &mut sqlx::postgres::PgConnection,
        limit: i64,
    ) -> Result<Vec<RecentOp>> {
        let ops = sqlx::query_as::<_, RecentOp>(
            "SELECT created_at, op_type, doc_number, product_name, operator_name FROM (\
             SELECT wr.created_at, '报工' AS op_type, wr.doc_number, \
               p.pdt_name AS product_name, u.display_name AS operator_name \
               FROM work_reports wr \
               LEFT JOIN work_orders wo ON wo.id = wr.work_order_id \
               LEFT JOIN products p ON p.product_id = wo.product_id \
               LEFT JOIN users u ON u.user_id = wr.operator_id \
             UNION ALL \
             SELECT pr.created_at, '完工入库', pr.doc_number, \
               p.pdt_name, u.display_name \
               FROM production_receipts pr \
               LEFT JOIN products p ON p.product_id = pr.product_id \
               LEFT JOIN users u ON u.user_id = pr.operator_id \
             UNION ALL \
             SELECT pi.created_at, '生产报检', pi.doc_number, \
               p.pdt_name, u.display_name \
               FROM production_inspections pi \
               LEFT JOIN products p ON p.product_id = pi.product_id \
               LEFT JOIN users u ON u.user_id = pi.inspector_id \
             ) ops ORDER BY created_at DESC LIMIT $1"
        ).bind(limit).fetch_all(&mut *executor).await?;
        Ok(ops)
    }
}
