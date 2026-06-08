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

    // ── Schedule Board ──

    pub async fn get_schedule_stats(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<ScheduleStats> {
        let stats = sqlx::query_as::<_, ScheduleStats>(
            "SELECT \
             (SELECT COUNT(*) FROM work_orders WHERE status IN (2,3)) AS active_orders, \
             (SELECT COUNT(*) FROM production_batches WHERE status = 1) AS pending_batches, \
             (SELECT COUNT(*) FROM production_batches WHERE status IN (2,3)) AS in_progress_batches, \
             (SELECT COUNT(*) FROM production_batches WHERE status = 4) AS pending_receipt_batches, \
             (SELECT COUNT(*) FROM production_batches WHERE status = 5) AS completed_batches"
        ).fetch_one(&mut *executor).await?;
        Ok(stats)
    }

    pub async fn get_schedule_cards(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<ScheduleCard>> {
        let cards = sqlx::query_as::<_, ScheduleCard>(
            "SELECT pb.id, pb.batch_no, pb.card_sn, \
             p.pdt_name AS product_name, \
             pb.batch_qty, pb.completed_qty, pb.current_step, \
             (SELECT COUNT(*)::int FROM work_order_routings WHERE work_order_id = pb.work_order_id) AS total_steps, \
             wor.process_name AS current_step_name, \
             pb.status, pb.work_order_id, \
             wo.doc_number AS wo_doc_number, \
             pb.created_at \
             FROM production_batches pb \
             LEFT JOIN products p ON p.product_id = pb.product_id \
             LEFT JOIN work_orders wo ON wo.id = pb.work_order_id \
             LEFT JOIN work_order_routings wor ON wor.work_order_id = pb.work_order_id AND wor.step_no = pb.current_step \
             WHERE pb.status != 6 \
             ORDER BY \
               CASE pb.status \
                 WHEN 2 THEN 1 WHEN 3 THEN 1 \
                 WHEN 1 THEN 2 \
                 WHEN 4 THEN 3 \
                 WHEN 5 THEN 4 \
               END, \
               pb.created_at DESC"
        ).fetch_all(&mut *executor).await?;
        Ok(cards)
    }

    pub async fn get_quick_entry_stats(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<QuickEntryStats> {
        let plan_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_plans")
            .fetch_one(&mut *executor)
            .await?;
        let order_active: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM work_orders WHERE status IN (2,3)")
                .fetch_one(&mut *executor)
                .await?;
        let batch_active: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM production_batches WHERE status IN (1,2,3,4)")
                .fetch_one(&mut *executor)
                .await?;
        let report_month: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM work_reports WHERE EXTRACT(MONTH FROM created_at) = EXTRACT(MONTH FROM CURRENT_DATE) AND EXTRACT(YEAR FROM created_at) = EXTRACT(YEAR FROM CURRENT_DATE)")
            .fetch_one(&mut *executor).await?;
        let insp_pending: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM production_inspections WHERE result = 0")
                .fetch_one(&mut *executor)
                .await?;
        let receipt_pending: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM production_receipts WHERE status = 1")
                .fetch_one(&mut *executor)
                .await?;
        let batch_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&mut *executor)
            .await?;
        let insp_total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM production_inspections WHERE result = 0")
                .fetch_one(&mut *executor)
                .await?;

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
             ) ops ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&mut *executor)
        .await?;
        Ok(ops)
    }

    // ── Material Usage ──

    pub async fn get_wo_basic_info(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<WoBasicInfo> {
        let info = sqlx::query_as::<_, WoBasicInfo>(
            "SELECT wo.id, wo.doc_number, wo.product_id, \
             p.pdt_name AS product_name, \
             wo.planned_qty, \
             COALESCE((SELECT SUM(pb.completed_qty) FROM production_batches pb WHERE pb.work_order_id = wo.id AND pb.status != 6), 0) AS completed_qty, \
             wo.status, wo.bom_snapshot_id, \
             b.bom_name AS bom_version \
             FROM work_orders wo \
             LEFT JOIN products p ON p.product_id = wo.product_id \
             LEFT JOIN boms b ON b.bom_id = wo.bom_snapshot_id \
             WHERE wo.id = $1"
        )
        .bind(work_order_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(info)
    }

    pub async fn get_bom_comparison(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<BomCompareItem>> {
        let items = sqlx::query_as::<_, BomCompareItem>(
            "SELECT bn.product_id AS component_id, \
             p.product_code AS component_code, \
             p.pdt_name AS component_name, \
             p.unit, \
             bn.quantity AS per_unit_qty, \
             bn.quantity * COALESCE(wo_batch.completed, 0) AS standard_total, \
             COALESCE(SUM(bi.actual_qty), 0) AS backflush_total, \
             COALESCE((SELECT SUM(mri.issued_qty) FROM material_requisition_items mri JOIN material_requisitions mr ON mr.id = mri.requisition_id WHERE mr.work_order_id = wo.id AND mri.product_id = bn.product_id AND mr.deleted_at IS NULL), 0) AS picked_qty \
             FROM work_orders wo \
             JOIN bom_nodes bn ON bn.bom_id = wo.bom_snapshot_id AND bn.parent_id != 0 \
             LEFT JOIN products p ON p.product_id = bn.product_id \
             LEFT JOIN (SELECT work_order_id, SUM(completed_qty) AS completed FROM production_batches WHERE status != 6 GROUP BY work_order_id) wo_batch ON wo_batch.work_order_id = wo.id \
             LEFT JOIN backflush_records br ON br.work_order_id = wo.id AND br.status = 2 \
             LEFT JOIN backflush_items bi ON bi.record_id = br.id AND bi.component_id = bn.product_id \
             WHERE wo.id = $1 \
             GROUP BY wo.id, bn.product_id, p.product_code, p.pdt_name, p.unit, bn.quantity, wo_batch.completed \
             ORDER BY p.pdt_name"
        )
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;
        Ok(items)
    }
}
