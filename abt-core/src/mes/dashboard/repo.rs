use chrono::NaiveDate;
use super::model::*;
use crate::shared::types::{DomainError, Result};

pub struct DashboardRepo;

impl DashboardRepo {
    pub async fn get_stats(executor: &mut sqlx::postgres::PgConnection) -> Result<DashboardStats> {
        let stats = sqlx::query_as::<_, DashboardStats>(
            "SELECT \
             (SELECT COUNT(*) FROM work_orders WHERE status IN (2,3)) AS active_order_count, \
             (SELECT COUNT(*) FROM production_batches WHERE status IN (1,2,3,4)) AS active_batch_count, \
             (SELECT COUNT(*) FROM stock_pickings WHERE picking_type = 2 AND status = 1 AND deleted_at IS NULL) AS pending_receipt_count, \
             (SELECT COALESCE(SUM(pi.qty_requested),0) FROM stock_pickings p \
                JOIN stock_picking_items pi ON pi.picking_id = p.id \
                WHERE p.picking_type = 2 AND p.status = 3 AND p.deleted_at IS NULL \
                AND EXTRACT(MONTH FROM p.created_at) = EXTRACT(MONTH FROM CURRENT_DATE) \
                AND EXTRACT(YEAR FROM p.created_at) = EXTRACT(YEAR FROM CURRENT_DATE)) AS completed_qty"
        ).fetch_one(&mut *executor).await?;
        Ok(stats)
    }

    pub async fn get_data_quality_stats(executor: &mut sqlx::postgres::PgConnection) -> Result<DataQualityStats> {
        let stats = sqlx::query_as::<_, DataQualityStats>(
            r#"WITH has_routing AS (
                SELECT DISTINCT br.product_code
                FROM bom_routings br
                JOIN routings r ON r.id = br.routing_id
                WHERE r.deleted_at IS NULL
            ), has_bom AS (
                SELECT DISTINCT bn.product_code
                FROM bom_nodes bn
                JOIN boms b ON b.bom_id = bn.bom_id
                WHERE bn.parent_id = 0 AND b.status = 2 AND b.deleted_at IS NULL
            )
            SELECT
              (SELECT COUNT(*) FROM products p
                WHERE NOT EXISTS (SELECT 1 FROM has_routing r WHERE r.product_code = p.product_code)) AS no_routing_count,
              (SELECT COUNT(*) FROM products p
                WHERE NOT EXISTS (SELECT 1 FROM has_bom b WHERE b.product_code = p.product_code)) AS no_bom_count,
              (SELECT COUNT(*) FROM products p
                WHERE EXISTS (SELECT 1 FROM has_routing r WHERE r.product_code = p.product_code)
                  AND EXISTS (SELECT 1 FROM has_bom b WHERE b.product_code = p.product_code)) AS complete_count"#,
        ).fetch_one(&mut *executor).await?;
        Ok(stats)
    }

    // ── Schedule Board ──

    pub async fn get_schedule_stats(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<ScheduleStats> {
        let stats = sqlx::query_as::<_, ScheduleStats>(
            "SELECT \
             (SELECT COUNT(*) FROM work_orders WHERE status IN (2,3,6)) AS active_orders, \
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
            sqlx::query_as("SELECT COUNT(*) FROM stock_pickings WHERE picking_type = 2 AND status = 1 AND deleted_at IS NULL")
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
             SELECT p.created_at, '完工入库', p.doc_number, \
               pdt.pdt_name, u.display_name \
               FROM stock_pickings p \
               LEFT JOIN LATERAL (SELECT product_id FROM stock_picking_items WHERE picking_id = p.id ORDER BY id LIMIT 1) pi ON true \
               LEFT JOIN products pdt ON pdt.product_id = pi.product_id \
               LEFT JOIN users u ON u.user_id = p.operator_id \
               WHERE p.picking_type = 2 AND p.deleted_at IS NULL \
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
 .fetch_optional(&mut *executor)
 .await?
 .ok_or_else(|| DomainError::NotFound(format!("工单 #{} 不存在", work_order_id)))?;
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
             COALESCE((SELECT SUM(spi.qty_done) FROM stock_picking_items spi JOIN stock_pickings sp ON sp.id = spi.picking_id WHERE sp.work_order_id = wo.id AND spi.product_id = bn.product_id AND sp.picking_type = 5 AND sp.deleted_at IS NULL AND sp.status <> 4), 0) AS picked_qty \
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

    // ── Gantt & Load (排程甘特图 & 负荷分析) ──

    /// 查询甘特图色块数据（booking JOIN 工单/产品/工序）
    pub async fn get_gantt_bookings(
        executor: &mut sqlx::postgres::PgConnection,
        work_center_ids: &[i64],
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<GanttBooking>> {
        let rows = sqlx::query_as::<_, GanttBooking>(
            r#"SELECT DISTINCT ON (b.id)
                   b.id AS booking_id, b.work_center_id,
                   b.date_from, b.date_to, b.duration_minutes,
                   b.work_order_id, wo.doc_number AS wo_doc_number,
                   b.plan_item_id,
                   p.pdt_name AS product_name,
                   pb.batch_no,
                   wor.process_name, wor.step_no AS step_order,
                   pb.status AS batch_status
               FROM work_center_bookings b
               JOIN work_orders wo ON wo.id = b.work_order_id
               LEFT JOIN production_batches pb ON pb.work_order_id = b.work_order_id
               LEFT JOIN products p ON p.product_id = pb.product_id
               LEFT JOIN work_order_routings wor
                     ON wor.work_order_id = b.work_order_id
                    AND wor.work_center_id = b.work_center_id
               WHERE b.work_center_id = ANY($1)
                 AND b.date_from < $3 AND b.date_to > $2
               ORDER BY b.id, wor.step_no"#,
        )
        .bind(work_center_ids)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *executor)
        .await?;
        Ok(rows)
    }

    /// 查询活跃工作中心列表（甘特图行头）
    pub async fn get_active_work_centers(
        executor: &mut sqlx::postgres::PgConnection,
        work_center_ids: Option<&[i64]>,
    ) -> Result<Vec<WorkCenterInfo>> {
        let rows = if let Some(ids) = work_center_ids {
            sqlx::query_as::<_, WorkCenterInfo>(
                "SELECT id, code, name, work_center_type \
                 FROM work_centers WHERE id = ANY($1) AND is_active \
                 ORDER BY code",
            )
            .bind(ids)
            .fetch_all(&mut *executor)
            .await?
        } else {
            sqlx::query_as::<_, WorkCenterInfo>(
                "SELECT id, code, name, work_center_type \
                 FROM work_centers WHERE is_active ORDER BY code",
            )
            .fetch_all(&mut *executor)
            .await?
        };
        Ok(rows)
    }

    /// 工作中心每日负荷（聚合 booking 工时 + 日历可用工时）
    /// 单条 SQL 用 generate_series + CTE 计算，返回完整 WcDailyLoad
    pub async fn get_work_center_load(
        executor: &mut sqlx::postgres::PgConnection,
        work_center_ids: &[i64],
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<WcDailyLoad>> {
        let rows = sqlx::query_as::<_, WcDailyLoad>(
            r#"WITH date_range AS (
                   SELECT generate_series($2, ($3 - 1), '1 day'::interval)::date AS d
               ),
               wc_days AS (
                   SELECT wc.id, wc.code, wc.name, wc.calendar_id, dr.d,
                          EXTRACT(DOW FROM dr.d)::int AS dow
                   FROM work_centers wc
                   CROSS JOIN date_range dr
                   WHERE wc.is_active AND wc.id = ANY($1)
               ),
               avail AS (
                   SELECT wd.id, wd.code, wd.name, wd.d,
                         COALESCE(SUM((EXTRACT(EPOCH FROM (cl.to_time - cl.from_time)) / 60)::numeric), 0) AS avail_mins
                   FROM wc_days wd
                   LEFT JOIN work_calendar_lines cl
                         ON cl.calendar_id = wd.calendar_id AND cl.weekday = wd.dow
                   GROUP BY wd.id, wd.code, wd.name, wd.d
               ),
               booked AS (
                   SELECT work_center_id, DATE(date_from) AS d,
                          SUM(duration_minutes) AS booked_mins
                   FROM work_center_bookings
                   WHERE work_center_id = ANY($1) AND date_from >= $2 AND date_from < $3
                   GROUP BY work_center_id, DATE(date_from)
               )
               SELECT a.id AS work_center_id, a.code AS work_center_code,
                      a.name AS work_center_name, a.d AS date,
                      COALESCE(b.booked_mins, 0) AS booked_minutes,
                      a.avail_mins AS available_minutes,
                      CASE WHEN a.avail_mins > 0
                           THEN ROUND(COALESCE(b.booked_mins, 0) * 100 / a.avail_mins, 1)
                           ELSE 0 END AS load_pct
               FROM avail a
               LEFT JOIN booked b ON b.work_center_id = a.id AND b.d = a.d
               ORDER BY a.id, a.d"#,
        )
        .bind(work_center_ids)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *executor)
        .await?;
        Ok(rows)
    }
}
