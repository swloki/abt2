use sqlx::FromRow;

use super::model::{
    CreateTransferItemReq, CreateTransferReq, InventoryTransfer, TransferFilter, TransferItem,
};
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::TransferStatus;

pub struct TransferRepo;

impl TransferRepo {
    /// 插入调拨单主表 + 明细，返回完整主记录
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreateTransferReq,
        operator_id: i64,
    ) -> Result<InventoryTransfer, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_transfers
                (doc_number, from_warehouse_id, from_zone_id, from_bin_id,
                 to_warehouse_id, to_zone_id, to_bin_id,
                 transfer_date, status, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, doc_number, from_warehouse_id, from_zone_id, from_bin_id,
                      to_warehouse_id, to_zone_id, to_bin_id,
                      transfer_date, status, operator_id, created_at
            "#,
        )
        .bind(doc_number)
        .bind(req.from_warehouse_id)
        .bind(req.from_zone_id)
        .bind(req.from_bin_id)
        .bind(req.to_warehouse_id)
        .bind(req.to_zone_id)
        .bind(req.to_bin_id)
        .bind(req.transfer_date)
        .bind(TransferStatus::Draft)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        let transfer = InventoryTransfer::from_row(&row)?;

        // 插入明细行
        for item in &req.items {
            Self::insert_item(&mut *executor, transfer.id, item).await?;
        }

        Ok(transfer)
    }

    /// 插入单条调拨明细
    async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        transfer_id: i64,
        item: &CreateTransferItemReq,
    ) -> Result<TransferItem, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO transfer_items
                (transfer_id, product_id, quantity, batch_no)
            VALUES ($1, $2, $3, $4)
            RETURNING id, transfer_id, product_id, quantity, batch_no
            "#,
        )
        .bind(transfer_id)
        .bind(item.product_id)
        .bind(item.quantity)
        .bind(&item.batch_no)
        .fetch_one(&mut *executor)
        .await?;

        TransferItem::from_row(&row)
    }

    /// 按 ID 查询调拨单
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<InventoryTransfer>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, from_warehouse_id, from_zone_id, from_bin_id,
                   to_warehouse_id, to_zone_id, to_bin_id,
                   transfer_date, status, operator_id, created_at
            FROM inventory_transfers
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| InventoryTransfer::from_row(&r)).transpose()
    }

    /// 查询调拨单的所有明细
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        transfer_id: i64,
    ) -> Result<Vec<TransferItem>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, transfer_id, product_id, quantity, batch_no
            FROM transfer_items
            WHERE transfer_id = $1
            ORDER BY id
            "#,
        )
        .bind(transfer_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| TransferItem::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    /// 更新调拨单状态，返回影响行数
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: TransferStatus,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_transfers
            SET status = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 分页查询调拨单列表
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &TransferFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransfer>, sqlx::Error> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 1u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.from_warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("from_warehouse_id = ${param_idx}"));
        }
        if filter.to_warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("to_warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM inventory_transfers WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, from_warehouse_id, from_zone_id, from_bin_id, \
             to_warehouse_id, to_zone_id, to_bin_id, \
             transfer_date, status, operator_id, created_at \
             FROM inventory_transfers WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.from_warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.to_warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<InventoryTransfer> = rows
            .iter()
            .filter_map(|r| InventoryTransfer::from_row(r).ok())
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
