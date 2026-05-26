//! ProductionBatch + WorkOrderRouting + WorkReport 数据访问层

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::*;
use super::super::enums::*;

// ===========================================================================
// ProductionBatchRepo
// ===========================================================================

pub struct ProductionBatchRepo;

impl ProductionBatchRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateBatchReq,
        batch_no: &str,
        card_sn: &str,
        operator_id: i64,
    ) -> Result<ProductionBatch> {
        let row = sqlx::query(
            r#"
            INSERT INTO production_batches
                (batch_no, card_sn, work_order_id, product_id, batch_qty,
                 completed_qty, scrap_qty, team_id, current_step,
                 status, operator_id)
            VALUES ($1, $2, $3, $4, $5, 0, 0, $6, 0, $7, $8)
            RETURNING id, batch_no, card_sn, work_order_id, product_id, batch_qty,
                      completed_qty, scrap_qty, team_id, current_step,
                      actual_start, actual_end, status, operator_id, created_at, updated_at
            "#,
        )
        .bind(batch_no)
        .bind(card_sn)
        .bind(req.work_order_id)
        .bind(req.product_id)
        .bind(req.batch_qty)
        .bind(req.team_id)
        .bind(BatchStatus::Pending)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ProductionBatch::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ProductionBatch>> {
        let row = sqlx::query(
            r#"
            SELECT id, batch_no, card_sn, work_order_id, product_id, batch_qty,
                   completed_qty, scrap_qty, team_id, current_step,
                   actual_start, actual_end, status, operator_id, created_at, updated_at
            FROM production_batches
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionBatch::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn list_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<ProductionBatch>> {
        let rows = sqlx::query(
            r#"
            SELECT id, batch_no, card_sn, work_order_id, product_id, batch_qty,
                   completed_qty, scrap_qty, team_id, current_step,
                   actual_start, actual_end, status, operator_id, created_at, updated_at
            FROM production_batches
            WHERE work_order_id = $1
            ORDER BY batch_no
            "#,
        )
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| ProductionBatch::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: BatchStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_batches
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }

    /// 更新批次当前工序序号，首道工序时自动将状态改为 InProgress
    pub async fn update_current_step(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        current_step: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_batches
            SET current_step = $2,
                status = CASE WHEN $2 = 1 THEN $3::smallint ELSE status END,
                actual_start = CASE WHEN $2 = 1 AND actual_start IS NULL THEN NOW() ELSE actual_start END,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(current_step)
        .bind(BatchStatus::InProgress)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }
}

// ===========================================================================
// WorkOrderRoutingRepo
// ===========================================================================

pub struct WorkOrderRoutingRepo;

impl WorkOrderRoutingRepo {
    /// 批量插入工单工序（工单级）
    pub async fn insert_for_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        steps: &[WorkOrderRouting],
    ) -> Result<()> {
        for step in steps {
            sqlx::query(
                r#"
                INSERT INTO work_order_routings
                    (work_order_id, step_no, process_name, work_center_id,
                     standard_time, standard_cost, unit_price, allowed_loss_rate,
                     planned_qty, completed_qty, defect_qty,
                     status, is_outsourced, is_inspection_point)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, 0, $10, $11, $12)
                "#,
            )
            .bind(step.work_order_id)
            .bind(step.step_no)
            .bind(&step.process_name)
            .bind(step.work_center_id)
            .bind(step.standard_time)
            .bind(step.standard_cost)
            .bind(step.unit_price)
            .bind(step.allowed_loss_rate)
            .bind(step.planned_qty)
            .bind(step.status)
            .bind(step.is_outsourced)
            .bind(step.is_inspection_point)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按工单 ID + 工序号查找工序
    pub async fn get_by_work_order_and_step(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
        step_no: i32,
    ) -> Result<Option<WorkOrderRouting>> {
        let row = sqlx::query(
            r#"
            SELECT id, work_order_id, step_no, process_name, work_center_id,
                   standard_time, standard_cost, unit_price, allowed_loss_rate,
                   planned_qty, completed_qty, defect_qty,
                   status, is_outsourced, is_inspection_point
            FROM work_order_routings
            WHERE work_order_id = $1 AND step_no = $2
            "#,
        )
        .bind(work_order_id)
        .bind(step_no)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| Ok(WorkOrderRouting::from_row(&r)?)).transpose()
    }

    /// 按工单 ID 查找所有工序（按 step_no 排序）
    pub async fn get_by_work_order_id(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderRouting>> {
        let rows = sqlx::query(
            r#"
            SELECT id, work_order_id, step_no, process_name, work_center_id,
                   standard_time, standard_cost, unit_price, allowed_loss_rate,
                   planned_qty, completed_qty, defect_qty,
                   status, is_outsourced, is_inspection_point
            FROM work_order_routings
            WHERE work_order_id = $1
            ORDER BY step_no
            "#,
        )
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| WorkOrderRouting::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    /// SQL 原子增量：累加 completed_qty 和 defect_qty
    pub async fn atomic_increment_qty(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        completed_delta: Decimal,
        defect_delta: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE work_order_routings
            SET completed_qty = completed_qty + $2,
                defect_qty = defect_qty + $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(completed_delta)
        .bind(defect_delta)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }

    /// 更新工序状态
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: RoutingStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE work_order_routings
            SET status = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }
}

// ===========================================================================
// WorkReportRepo（写操作，用于 confirm_routing_step）
// ===========================================================================

pub struct WorkReportRepo;

/// 报工记录行（本地查询模型）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkReportRow {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub report_date: NaiveDate,
    pub shift: ShiftType,
    pub worker_id: i64,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub work_hours: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl WorkReportRepo {
    /// 幂等插入报工记录
    pub async fn insert_or_get_existing(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        work_order_id: i64,
        batch_id: i64,
        routing_id: i64,
        report_date: NaiveDate,
        shift: ShiftType,
        worker_id: i64,
        completed_qty: Decimal,
        defect_qty: Decimal,
        defect_reason: Option<DefectReason>,
        work_hours: Decimal,
        remark: &str,
        operator_id: i64,
    ) -> Result<(WorkReportRow, bool)> {
        let row = sqlx::query(
            r#"
            INSERT INTO work_reports
                (doc_number, work_order_id, batch_id, routing_id, report_date,
                 shift, worker_id, completed_qty, defect_qty, defect_reason,
                 work_hours, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (batch_id, routing_id, worker_id, shift, report_date)
            DO NOTHING
            RETURNING id, doc_number, work_order_id, batch_id, routing_id, report_date,
                      shift, worker_id, completed_qty, defect_qty, defect_reason,
                      work_hours, remark, operator_id, created_at, updated_at
            "#,
        )
        .bind(doc_number)
        .bind(work_order_id)
        .bind(batch_id)
        .bind(routing_id)
        .bind(report_date)
        .bind(shift)
        .bind(worker_id)
        .bind(completed_qty)
        .bind(defect_qty)
        .bind(defect_reason)
        .bind(work_hours)
        .bind(remark)
        .bind(operator_id)
        .fetch_optional(&mut *executor)
        .await?;

        match row {
            Some(r) => {
                let report = WorkReportRow::from_row(&r)?;
                Ok((report, true))
            }
            None => {
                let existing = sqlx::query(
                    r#"
                    SELECT id, doc_number, work_order_id, batch_id, routing_id, report_date,
                           shift, worker_id, completed_qty, defect_qty, defect_reason,
                           work_hours, remark, operator_id, created_at, updated_at
                    FROM work_reports
                    WHERE batch_id = $1 AND routing_id = $2 AND worker_id = $3
                          AND shift = $4 AND report_date = $5
                    "#,
                )
                .bind(batch_id)
                .bind(routing_id)
                .bind(worker_id)
                .bind(shift)
                .bind(report_date)
                .fetch_one(&mut *executor)
                .await?;

                let report = WorkReportRow::from_row(&existing)?;
                Ok((report, false))
            }
        }
    }
}
