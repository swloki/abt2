use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{
    ArrivalNotice, ArrivalNoticeItem, CreateArrivalNoticeItemReq, CreateArrivalNoticeReq,
    ArrivalNoticeFilter,
};
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::ArrivalStatus;

pub struct ArrivalNoticeRepo;

impl ArrivalNoticeRepo {
    /// INSERT 来料通知主表 + 明细，返回生成的实体
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreateArrivalNoticeReq,
        operator_id: i64,
    ) -> Result<ArrivalNotice> {
        let row = sqlx::query(
            r#"
            INSERT INTO arrival_notices
                (doc_number, purchase_order_id, supplier_id, arrival_date, status,
                 warehouse_id, zone_id, delivery_note, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, doc_number, purchase_order_id, supplier_id, arrival_date, status,
                      warehouse_id, zone_id, delivery_note, remark, operator_id,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(doc_number)
        .bind(req.purchase_order_id)
        .bind(req.supplier_id)
        .bind(req.arrival_date)
        .bind(ArrivalStatus::Draft)
        .bind(req.warehouse_id)
        .bind(req.zone_id)
        .bind(&req.delivery_note)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        let notice = ArrivalNotice::from_row(&row)?;

        // 批量插入明细行
        for item in &req.items {
            Self::insert_item(&mut *executor, notice.id, item).await?;
        }

        Ok(notice)
    }

    /// INSERT 单条来料通知明细
    async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        notice_id: i64,
        item: &CreateArrivalNoticeItemReq,
    ) -> Result<ArrivalNoticeItem> {
        let row = sqlx::query(
            r#"
            INSERT INTO arrival_notice_items
                (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
            VALUES ($1, $2, $3, $4, 0, 0, $5)
            RETURNING id, notice_id, order_item_id, product_id, declared_qty,
                      received_qty, accepted_qty, batch_no
            "#,
        )
        .bind(notice_id)
        .bind(item.order_item_id)
        .bind(item.product_id)
        .bind(item.declared_qty)
        .bind(&item.batch_no)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ArrivalNoticeItem::from_row(&row)?)
    }

    /// 按 ID 查询来料通知（排除已软删除）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ArrivalNotice>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, purchase_order_id, supplier_id, arrival_date, status,
                   warehouse_id, zone_id, delivery_note, remark, operator_id,
                   created_at, updated_at, deleted_at
            FROM arrival_notices
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ArrivalNotice::from_row(&r).map_err(Into::into)).transpose()

    }

    /// 查询来料通知的所有明细行
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        notice_id: i64,
    ) -> Result<Vec<ArrivalNoticeItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, notice_id, order_item_id, product_id, declared_qty,
                   received_qty, accepted_qty, batch_no
            FROM arrival_notice_items
            WHERE notice_id = $1
            "#,
        )
        .bind(notice_id)
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| ArrivalNoticeItem::from_row(r).ok())
            .collect())
    }

    /// 更新来料通知状态
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: ArrivalStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE arrival_notices
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

    /// 更新明细行收货数量和批次号
    pub async fn update_item_received(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        received_qty: Decimal,
        batch_no: Option<&str>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE arrival_notice_items
            SET received_qty = $2, batch_no = COALESCE($3, batch_no)
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .bind(received_qty)
        .bind(batch_no)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新明细行检验通过数量
    pub async fn update_item_accepted(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        accepted_qty: Decimal,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE arrival_notice_items
            SET accepted_qty = $2
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .bind(accepted_qty)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 分页查询来料通知，支持按状态/供应商/仓库过滤
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &ArrivalNoticeFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ArrivalNotice>> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.supplier_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("supplier_id = ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM arrival_notices WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, purchase_order_id, supplier_id, arrival_date, status, \
             warehouse_id, zone_id, delivery_note, remark, operator_id, \
             created_at, updated_at, deleted_at \
             FROM arrival_notices WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.supplier_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<ArrivalNotice> = rows
            .iter()
            .filter_map(|r| ArrivalNotice::from_row(r).ok())
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

    /// 软删除来料通知（用于取消）
    pub async fn soft_delete(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE arrival_notices SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }
}
