//! ProductionBatch + WorkOrderRouting + WorkReport 数据访问层

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

    pub async fn find_by_card_sn(
        executor: &mut sqlx::postgres::PgConnection,
        card_sn: &str,
    ) -> Result<Option<ProductionBatch>> {
        let row = sqlx::query(
            r#"
            SELECT id, batch_no, card_sn, work_order_id, product_id, batch_qty,
                   completed_qty, scrap_qty, team_id, current_step,
                   actual_start, actual_end, status, operator_id, created_at, updated_at
            FROM production_batches
            WHERE card_sn = $1
            "#,
        )
        .bind(card_sn)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionBatch::from_row(&r).map_err(Into::into)).transpose()
    }

    pub async fn list_batches(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &BatchListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<BatchListItem>, i64)> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: i32 = 0;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pb.status = ${param_idx}"));
        }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() {
                param_idx += 1;
                where_clauses.push(format!("pb.batch_no ILIKE ${param_idx}"));
            }
        if let Some(won) = &filter.work_order_no
            && !won.is_empty() {
                param_idx += 1;
                where_clauses.push(format!("wo.doc_number ILIKE ${param_idx}"));
            }

        let where_sql = where_clauses.join(" AND ");

        // Count query（LEFT JOIN work_orders：work_order_no 筛选需要 wo.doc_number）
        let count_sql = format!(
            "SELECT COUNT(*)::bigint FROM production_batches pb \
             LEFT JOIN work_orders wo ON wo.id = pb.work_order_id WHERE {where_sql}"
        );
        let mut count_query = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(st) = filter.status {
            count_query = count_query.bind(st.as_i16());
        }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() {
                count_query = count_query.bind(format!("%{kw}%"));
            }
        if let Some(won) = &filter.work_order_no
            && !won.is_empty() {
                count_query = count_query.bind(format!("%{won}%"));
            }
        let total: i64 = count_query.fetch_one(&mut *executor).await?;

        // Data query
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;
        let data_sql = format!(
            "SELECT pb.id, pb.batch_no, pb.card_sn, pb.work_order_id, wo.doc_number AS wo_doc_number, \
             pb.product_id, p.pdt_name AS product_name, pb.batch_qty, pb.completed_qty, pb.scrap_qty, pb.current_step, \
             wor.process_name AS current_step_name, \
             (SELECT COUNT(*)::int FROM work_order_routings WHERE work_order_id = pb.work_order_id) AS total_steps, \
             pb.status, pb.created_at \
             FROM production_batches pb \
             LEFT JOIN work_orders wo ON wo.id = pb.work_order_id \
             LEFT JOIN products p ON p.product_id = pb.product_id \
             LEFT JOIN work_order_routings wor ON wor.work_order_id = pb.work_order_id AND wor.step_no = pb.current_step \
             WHERE {where_sql} \
             ORDER BY pb.created_at DESC \
             LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );
        let mut data_query = sqlx::query_as::<sqlx::Postgres, BatchListItem>(sqlx::AssertSqlSafe(data_sql));
        if let Some(st) = filter.status {
            data_query = data_query.bind(st.as_i16());
        }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() {
                data_query = data_query.bind(format!("%{kw}%"));
            }
        if let Some(won) = &filter.work_order_no
            && !won.is_empty() {
                data_query = data_query.bind(format!("%{won}%"));
            }
        data_query = data_query.bind(page_size as i64).bind(offset as i64);
        let items = data_query.fetch_all(&mut *executor).await?;

        Ok((items, total))
    }

    /// 行锁原子累加批次完成量/报废量（报工事务内调用）
    pub async fn atomic_increment_qty(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        completed_delta: Decimal,
        scrap_delta: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_batches
            SET completed_qty = completed_qty + $2,
                scrap_qty = scrap_qty + $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(completed_delta)
        .bind(scrap_delta)
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
                   planned_qty, is_outsourced, is_inspection_point, product_id
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

    /// 按 id 查找工序（带 work_order_id 用于越权校验）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<Option<WorkOrderRouting>> {
        let row = sqlx::query(
            r#"
            SELECT id, work_order_id, step_no, process_name, work_center_id,
                   standard_time, standard_cost, unit_price, allowed_loss_rate,
                   planned_qty, is_outsourced, is_inspection_point, product_id
            FROM work_order_routings
            WHERE id = $1
            "#,
        )
        .bind(routing_id)
        .fetch_optional(&mut *executor)
        .await?;
        row.map(|r| Ok(WorkOrderRouting::from_row(&r)?)).transpose()
    }

    /// 删除单条工序
    pub async fn delete(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<()> {
        sqlx::query(r#"DELETE FROM work_order_routings WHERE id = $1"#)
            .bind(routing_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// 删除后重排：剩余工序 step_no 压成 1..N 连续
    pub async fn renumber_steps(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<()> {
        // 两步法：先移到负数安全区避免 step_no UNIQUE 约束在 UPDATE 时瞬时冲突
        sqlx::query(
            "UPDATE work_order_routings SET step_no = -(step_no + 1) WHERE work_order_id = $1",
        )
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;
        // 再按当前（负值）顺序赋 0-based 连续号
        sqlx::query(
            r#"
            WITH ordered AS (
                SELECT id, ROW_NUMBER() OVER (ORDER BY -step_no) - 1 AS new_no
                FROM work_order_routings
                WHERE work_order_id = $1
            )
            UPDATE work_order_routings wor
            SET step_no = ordered.new_no
            FROM ordered
            WHERE wor.id = ordered.id
            "#,
        )
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 该工序是否已有报工记录（改价逐行守卫）
    pub async fn has_report(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
    ) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM work_reports WHERE routing_id = $1)"#,
        )
        .bind(routing_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(exists.0)
    }

    /// 该工单是否有任意报工记录（删除全局守卫）
    pub async fn has_any_report(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM work_reports wr
                JOIN work_order_routings wor ON wor.id = wr.routing_id
                WHERE wor.work_order_id = $1
            )
            "#,
        )
        .bind(work_order_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(exists.0)
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
                   planned_qty, is_outsourced, is_inspection_point, product_id
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
            .map(Ok)
            .collect()
    }

    /// 批量按工单 ID 查找工序（N+1 修复：calculate_wage 用）
    pub async fn get_by_work_order_ids(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_ids: &[i64],
    ) -> Result<Vec<WorkOrderRouting>> {
        if work_order_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT id, work_order_id, step_no, process_name, work_center_id,
                   standard_time, standard_cost, unit_price, allowed_loss_rate,
                   planned_qty, is_outsourced, is_inspection_point, product_id
            FROM work_order_routings
            WHERE work_order_id = ANY($1)
            ORDER BY step_no
            "#,
        )
        .bind(work_order_ids)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| WorkOrderRouting::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }
}

// ===========================================================================
// BatchRoutingProgressRepo — 批次工序执行进度（写真相源）
// ===========================================================================

pub struct BatchRoutingProgressRepo;

impl BatchRoutingProgressRepo {
    /// UPSERT (batch_id, routing_id) 记录，返回 id
    pub async fn upsert_and_get_id(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
        routing_id: i64,
    ) -> Result<i64> {
        let row: Option<i64> = sqlx::query_scalar(
            r#"
            INSERT INTO batch_routing_progress (batch_id, routing_id)
            VALUES ($1, $2)
            ON CONFLICT (batch_id, routing_id) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(batch_id)
        .bind(routing_id)
        .fetch_optional(&mut *executor)
        .await?;

        if let Some(id) = row {
            Ok(id)
        } else {
            let id: i64 = sqlx::query_scalar(
                "SELECT id FROM batch_routing_progress WHERE batch_id = $1 AND routing_id = $2",
            )
            .bind(batch_id)
            .bind(routing_id)
            .fetch_one(&mut *executor)
            .await?;
            Ok(id)
        }
    }

    /// 行锁原子累加 completed_qty / defect_qty
    pub async fn atomic_increment_qty(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        completed_delta: Decimal,
        defect_delta: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE batch_routing_progress
            SET completed_qty = completed_qty + $2,
                defect_qty = defect_qty + $3,
                updated_at = NOW()
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

    /// 更新状态（首次报工 → InProgress，末道工序 → Completed）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: RoutingStatus,
    ) -> Result<()> {
        let sql = match status {
            RoutingStatus::InProgress => {
                "UPDATE batch_routing_progress SET status = $2, started_at = COALESCE(started_at, NOW()), updated_at = NOW() WHERE id = $1"
            }
            RoutingStatus::Completed => {
                "UPDATE batch_routing_progress SET status = $2, completed_at = NOW(), updated_at = NOW() WHERE id = $1"
            }
            _ => {
                "UPDATE batch_routing_progress SET status = $2, updated_at = NOW() WHERE id = $1"
            }
        };
        sqlx::query(sql)
            .bind(id)
            .bind(status)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// 按 (batch_id, routing_id) 查单条
    pub async fn get_by_batch_and_routing(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
        routing_id: i64,
    ) -> Result<Option<BatchRoutingProgress>> {
        let row = sqlx::query(
            r#"
            SELECT id, batch_id, routing_id, status, completed_qty, defect_qty,
                   started_at, completed_at, created_at, updated_at
            FROM batch_routing_progress
            WHERE batch_id = $1 AND routing_id = $2
            "#,
        )
        .bind(batch_id)
        .bind(routing_id)
        .fetch_optional(&mut *executor)
        .await?;
        row.map(|r| Ok(BatchRoutingProgress::from_row(&r)?)).transpose()
    }

    /// 按工单查所有批次工序进度（矩阵一次查全，避免 list_by_batch × N 批次 N+1）
    pub async fn list_progress_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<BatchRoutingProgress>> {
        let rows = sqlx::query(
            r#"
            SELECT p.id, p.batch_id, p.routing_id, p.status, p.completed_qty, p.defect_qty,
                   p.started_at, p.completed_at, p.created_at, p.updated_at
            FROM batch_routing_progress p
            JOIN production_batches b ON b.id = p.batch_id
            WHERE b.work_order_id = $1
            "#,
        )
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;
        rows.iter()
            .filter_map(|r| BatchRoutingProgress::from_row(r).ok())
            .map(Ok)
            .collect()
    }

    /// 按批次查所有工序进度
    pub async fn list_by_batch(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
    ) -> Result<Vec<BatchRoutingProgress>> {
        let rows = sqlx::query(
            r#"
            SELECT id, batch_id, routing_id, status, completed_qty, defect_qty,
                   started_at, completed_at, created_at, updated_at
            FROM batch_routing_progress
            WHERE batch_id = $1
            ORDER BY (SELECT step_no FROM work_order_routings WHERE id = routing_id)
            "#,
        )
        .bind(batch_id)
        .fetch_all(&mut *executor)
        .await?;
        rows.iter()
            .filter_map(|r| BatchRoutingProgress::from_row(r).ok())
            .map(Ok)
            .collect()
    }

    /// 为新批次初始化所有工序的 progress 记录
    pub async fn init_for_batch(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
        work_order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO batch_routing_progress (batch_id, routing_id)
            SELECT $1, id FROM work_order_routings WHERE work_order_id = $2
            ON CONFLICT (batch_id, routing_id) DO NOTHING
            "#,
        )
        .bind(batch_id)
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}

// ===========================================================================
// WorkReportRepo（写操作，用于 confirm_routing_step）
// ===========================================================================

pub struct WorkReportRepo;

impl WorkReportRepo {
    /// 插入报工记录，返回 (id, was_inserted)。
    /// 原 ON CONFLICT 幂等约束已移除（migration 063），允许同工人同班次同天分批报工；
    /// 每次报工都插入新记录并累加，防重复提交由前端 + 后端事务保证。
    pub async fn insert_or_get_existing(
        executor: &mut sqlx::postgres::PgConnection,
        params: &InsertWorkReportParams<'_>,
    ) -> Result<(i64, bool)> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO work_reports
                (doc_number, work_order_id, batch_id, routing_id, report_date,
                 shift, worker_id, completed_qty, defect_qty, defect_reason,
                 work_hours, remark, operator_id, wage_amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(params.doc_number)
        .bind(params.work_order_id)
        .bind(params.batch_id)
        .bind(params.routing_id)
        .bind(params.report_date)
        .bind(params.shift)
        .bind(params.worker_id)
        .bind(params.completed_qty)
        .bind(params.defect_qty)
        .bind(params.defect_reason)
        .bind(params.work_hours)
        .bind(params.remark)
        .bind(params.operator_id)
        .bind(params.wage_amount)
        .fetch_one(&mut *executor)
        .await?;

        Ok((id, true))
    }
}
