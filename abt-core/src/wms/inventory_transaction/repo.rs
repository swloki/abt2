use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};
use crate::shared::types::pagination::PaginatedResult;

pub struct InventoryTransactionRepo;

impl InventoryTransactionRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &RecordTransactionReq,
        operator_id: i64,
    ) -> Result<InventoryTransaction> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_transactions
                (doc_number, delivery_no, source_doc_number, transaction_type, product_id, warehouse_id, zone_id, bin_id,
                 batch_no, quantity, unit_cost, source_type, source_id, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING id, doc_number, delivery_no, source_doc_number, transaction_type, product_id, warehouse_id, zone_id,
                      bin_id, batch_no, quantity, unit_cost, source_type, source_id,
                      remark, operator_id, created_at
            "#,
        )
        .bind(&req.doc_number)
        .bind(&req.delivery_no)
        .bind(&req.source_doc_number)
        .bind(req.transaction_type)
        .bind(req.product_id)
        .bind(req.warehouse_id)
        .bind(req.zone_id)
        .bind(req.bin_id)
        .bind(&req.batch_no)
        .bind(req.quantity)
        .bind(req.unit_cost)
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(InventoryTransaction::from_row(&row)?)
    }

    pub async fn find_by_source(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: &str,
        source_id: i64,
    ) -> Result<Vec<InventoryTransaction>> {
        let rows = sqlx::query(
            r#"
            SELECT id, doc_number, delivery_no, source_doc_number, transaction_type, product_id, warehouse_id, zone_id,
                   bin_id, batch_no, quantity, unit_cost, source_type, source_id,
                   remark, operator_id, created_at
            FROM inventory_transactions
            WHERE source_type = $1 AND source_id = $2
            ORDER BY created_at
            "#,
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_all(executor)
        .await?;

        rows.iter()
            .map(|r| InventoryTransaction::from_row(r).map_err(Into::into))
            .collect()
    }


    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<InventoryTransaction>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, delivery_no, source_doc_number, transaction_type, product_id, warehouse_id, zone_id,
                   bin_id, batch_no, quantity, unit_cost, source_type, source_id,
                   remark, operator_id, created_at
            FROM inventory_transactions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(row.as_ref().and_then(|r| InventoryTransaction::from_row(r).ok()))
    }
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &TransactionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransaction>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if filter.transaction_type.is_some() { param_idx += 1; conditions.push(format!("transaction_type = ${param_idx}")); }
        if filter.product_id.is_some() { param_idx += 1; conditions.push(format!("product_id = ${param_idx}")); }
        if filter.warehouse_id.is_some() { param_idx += 1; conditions.push(format!("warehouse_id = ${param_idx}")); }
        if filter.source_type.is_some() { param_idx += 1; conditions.push(format!("source_type = ${param_idx}")); }
        if filter.source_id.is_some() { param_idx += 1; conditions.push(format!("source_id = ${param_idx}")); }
        if filter.doc_number.is_some() { param_idx += 1; conditions.push(format!("doc_number ILIKE ${param_idx}")); }
        if filter.product_code.is_some() {
            param_idx += 1;
            conditions.push(format!("product_id IN (SELECT product_id FROM products WHERE product_code ILIKE ${param_idx})"));
        }

        let where_sql = if conditions.is_empty() { "1=1".to_string() } else { conditions.join(" AND ") };
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM inventory_transactions WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, delivery_no, source_doc_number, transaction_type, product_id, warehouse_id, zone_id, \
             bin_id, batch_no, quantity, unit_cost, source_type, source_id, \
             remark, operator_id, created_at \
             FROM inventory_transactions WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = filter.transaction_type { count_q = count_q.bind(v); }
        if let Some(v) = filter.product_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.warehouse_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.source_type { count_q = count_q.bind(v); }
        if let Some(v) = filter.source_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.doc_number { count_q = count_q.bind(format!("%{v}%")); }
        if let Some(ref v) = filter.product_code { count_q = count_q.bind(format!("%{v}%")); }

        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = filter.transaction_type { data_q = data_q.bind(v); }
        if let Some(v) = filter.product_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.warehouse_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.source_type { data_q = data_q.bind(v); }
        if let Some(v) = filter.source_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.doc_number { data_q = data_q.bind(format!("%{v}%")); }
        if let Some(ref v) = filter.product_code { data_q = data_q.bind(format!("%{v}%")); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<InventoryTransaction> = rows.iter().filter_map(|r| InventoryTransaction::from_row(r).ok()).collect();

        let total_pages = (total as u64).div_ceil(page_size as u64) as u32;

        Ok(PaginatedResult { items, total: total as u64, page, page_size, total_pages })
    }
}
