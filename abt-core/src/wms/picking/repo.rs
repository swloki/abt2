use sqlx::FromRow;

use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::Result;
use crate::wms::enums::PickingStatus;

use super::model::{
    CreatePickingItemReq, CreatePickingReq, PickingFilter, StockPicking, StockPickingItem,
};

pub struct PickingRepo;

impl PickingRepo {
    /// 插入作业单据主表 + 明细，返回完整主记录
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreatePickingReq,
        operator_id: i64,
    ) -> Result<StockPicking> {
        let source_type = req.source_type.as_deref().unwrap_or("none");
        let row = sqlx::query(
            r#"
            INSERT INTO stock_pickings
                (doc_number, picking_type, status, source_type, source_id, partner_id,
                 from_warehouse_id, from_zone_id, from_bin_id,
                 to_warehouse_id, to_zone_id, to_bin_id,
                 operator_id, scheduled_date, work_order_id, remark)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id, doc_number, picking_type, status, source_type, source_id, partner_id,
                      from_warehouse_id, from_zone_id, from_bin_id,
                      to_warehouse_id, to_zone_id, to_bin_id,
                      operator_id, scheduled_date, done_at, pick_list_id, work_order_id, remark,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(doc_number)
        .bind(req.picking_type)
        .bind(PickingStatus::Draft)
        .bind(source_type)
        .bind(req.source_id)
        .bind(req.partner_id)
        .bind(req.from_warehouse_id)
        .bind(req.from_zone_id)
        .bind(req.from_bin_id)
        .bind(req.to_warehouse_id)
        .bind(req.to_zone_id)
        .bind(req.to_bin_id)
        .bind(operator_id)
        .bind(req.scheduled_date)
        .bind(req.work_order_id)
        .bind(req.remark.as_deref().unwrap_or(""))
        .fetch_one(&mut *executor)
        .await?;

        let picking = StockPicking::from_row(&row)?;

        for item in &req.items {
            Self::insert_item(&mut *executor, picking.id, item).await?;
        }

        Ok(picking)
    }

    async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        picking_id: i64,
        item: &CreatePickingItemReq,
    ) -> Result<StockPickingItem> {
        let row = sqlx::query(
            r#"
            INSERT INTO stock_picking_items
                (picking_id, product_id, batch_no, qty_requested, qty_done,
                 from_bin_id, to_bin_id, operation_id, batch_id, source_item_id, remark)
            VALUES ($1, $2, $3, $4, 0, $5, $6, $7, $8, $9, $10)
            RETURNING id, picking_id, product_id, batch_no, qty_requested, qty_done,
                      from_bin_id, to_bin_id, operation_id, batch_id, source_item_id, remark, created_at
            "#,
        )
        .bind(picking_id)
        .bind(item.product_id)
        .bind(&item.batch_no)
        .bind(item.qty_requested)
        .bind(item.from_bin_id)
        .bind(item.to_bin_id)
        .bind(item.operation_id)
        .bind(item.batch_id)
        .bind(item.source_item_id)
        .bind(item.remark.as_deref().unwrap_or(""))
        .fetch_one(&mut *executor)
        .await?;

        Ok(StockPickingItem::from_row(&row)?)
    }

    /// 按 ID 查询作业单据（自动过滤软删除）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<StockPicking>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, picking_type, status, source_type, source_id, partner_id,
                   from_warehouse_id, from_zone_id, from_bin_id,
                   to_warehouse_id, to_zone_id, to_bin_id,
                   operator_id, scheduled_date, done_at, pick_list_id, work_order_id, remark,
                   created_at, updated_at, deleted_at
            FROM stock_pickings
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| StockPicking::from_row(&r).map_err(Into::into))
            .transpose()
    }

    /// 查询作业单据的所有明细
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        picking_id: i64,
    ) -> Result<Vec<StockPickingItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, picking_id, product_id, batch_no, qty_requested, qty_done,
                   from_bin_id, to_bin_id, operation_id, batch_id, source_item_id, remark, created_at
            FROM stock_picking_items
            WHERE picking_id = $1
            ORDER BY id
            "#,
        )
        .bind(picking_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| StockPickingItem::from_row(r).map_err(Into::into))
            .collect()
    }

    /// 批量查多个作业单据的明细（避免 N+1）
    pub async fn get_items_by_picking_ids(
        executor: &mut sqlx::postgres::PgConnection,
        picking_ids: &[i64],
    ) -> Result<Vec<StockPickingItem>> {
        if picking_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT id, picking_id, product_id, batch_no, qty_requested, qty_done,
                   from_bin_id, to_bin_id, operation_id, batch_id, source_item_id, remark, created_at
            FROM stock_picking_items
            WHERE picking_id = ANY($1)
            ORDER BY id
            "#,
        )
        .bind(picking_ids)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| StockPickingItem::from_row(r).map_err(Into::into))
            .collect()
    }

    /// 更新作业单据状态，返回影响行数
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: PickingStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE stock_pickings
            SET status = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新发货仓库（OutgoingSales direct_ship 选仓：销售申请时 from_warehouse=None，发货时填入）
    pub async fn update_from_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        warehouse_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE stock_pickings
            SET from_warehouse_id = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(warehouse_id)
        .execute(&mut *executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 标记完成：status = Done + done_at = NOW()
    pub async fn set_done(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE stock_pickings
            SET status = $2, done_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(PickingStatus::Done)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新明细行级实绩（done / issue 时按行写回 qty_done + 库位）
    pub async fn update_item_done(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        qty_done: rust_decimal::Decimal,
        batch_no: Option<&str>,
        from_bin_id: Option<i64>,
        to_bin_id: Option<i64>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE stock_picking_items
            SET qty_done = $2, batch_no = COALESCE($3, batch_no),
                from_bin_id = COALESCE($4, from_bin_id), to_bin_id = COALESCE($5, to_bin_id)
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .bind(qty_done)
        .bind(batch_no)
        .bind(from_bin_id)
        .bind(to_bin_id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询批次已领料的工序 routing_id 集合（驱动 MES 批次矩阵动作位推进）。
    /// 仅 InternalIssue + 未取消的 picking。
    pub async fn find_routing_ids_by_batch(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
    ) -> Result<Vec<i64>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT i.operation_id
            FROM stock_picking_items i
            JOIN stock_pickings p ON p.id = i.picking_id
            WHERE i.batch_id = $1
              AND i.operation_id IS NOT NULL
              AND p.picking_type = $2
              AND p.deleted_at IS NULL
              AND p.status <> $3
            "#,
        )
        .bind(batch_id)
        .bind(crate::wms::enums::PickingType::InternalIssue)
        .bind(PickingStatus::Cancelled)
        .fetch_all(executor)
        .await?;
        Ok(ids)
    }

    /// 分页查询作业单据列表（自动过滤软删除）
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &PickingFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockPicking>> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["p.deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        if filter.doc_number.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.doc_number ILIKE ${param_idx}"));
        }
        if filter.picking_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.picking_type = ${param_idx}"));
        }
        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.status = ${param_idx}"));
        }
        if filter.source_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.source_type = ${param_idx}"));
        }
        if filter.source_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.source_id = ${param_idx}"));
        }
        if filter.work_order_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("p.work_order_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) FROM stock_pickings p WHERE {where_sql}");
        let data_sql = format!(
            "SELECT p.id, p.doc_number, p.picking_type, p.status, p.source_type, p.source_id, p.partner_id, \
             p.from_warehouse_id, p.from_zone_id, p.from_bin_id, \
             p.to_warehouse_id, p.to_zone_id, p.to_bin_id, \
             p.operator_id, p.scheduled_date, p.done_at, p.pick_list_id, p.work_order_id, p.remark, \
             p.created_at, p.updated_at, p.deleted_at, \
             (SELECT COUNT(*) FROM stock_picking_items pi WHERE pi.picking_id = p.id) AS item_count \
             FROM stock_pickings p WHERE {where_sql} \
             ORDER BY p.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));

        if let Some(ref v) = filter.doc_number {
            let pattern = format!("%{v}%");
            count_q = count_q.bind(pattern.clone());
            data_q = data_q.bind(pattern);
        }
        if let Some(v) = filter.picking_type {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(ref v) = filter.source_type {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.source_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.work_order_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<StockPicking> = rows
            .iter()
            .filter_map(|r| StockPicking::from_row(r).ok())
            .collect();

        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page_size as u64) as u32
        };

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }
}
