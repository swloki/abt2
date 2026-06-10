use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{InventoryDetailView, TransactionDetailView, TransactionLogFilter};
use crate::shared::types::pagination::PaginatedResult;

pub struct InventoryRepo;

impl InventoryRepo {
    // ── 库存详情 JOIN 查询 ──

    pub async fn query_stock_details(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: Option<i64>,
        keyword: Option<&str>,
        warehouse_id: Option<i64>,
        bin_id: Option<i64>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryDetailView>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if product_id.is_some() {
            param_idx += 1;
            conditions.push(format!("sl.product_id = ${param_idx}"));
        }
        if keyword.is_some() {
            param_idx += 1;
            conditions.push(format!("(p.pdt_name ILIKE ${param_idx} OR p.product_code ILIKE ${param_idx})"));
        }
        if warehouse_id.is_some() {
            param_idx += 1;
            conditions.push(format!("sl.warehouse_id = ${param_idx}"));
        }
        if bin_id.is_some() {
            param_idx += 1;
            conditions.push(format!("sl.bin_id = ${param_idx}"));
        }

        let where_sql = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let select_clause = "sl.id as stock_ledger_id, sl.product_id, p.pdt_name as product_name, \
             p.product_code, sl.warehouse_id, w.name as warehouse_name, \
             sl.bin_id, b.code as bin_code, sl.quantity, sl.safety_stock";

        let from_clause = "stock_ledger sl \
             JOIN products p ON sl.product_id = p.product_id \
             JOIN bins b ON sl.bin_id = b.id \
             JOIN zones z ON b.zone_id = z.id \
             JOIN warehouses w ON z.warehouse_id = w.id";

        let count_sql = format!("SELECT COUNT(*) as total FROM {from_clause} WHERE 1=1 {where_sql}");
        let data_sql = format!(
            "SELECT {select_clause} FROM {from_clause} WHERE 1=1 {where_sql} \
             ORDER BY sl.updated_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = product_id { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword {
            let pattern = format!("%{v}%");
            count_q = count_q.bind(pattern);
        }
        if let Some(v) = warehouse_id { count_q = count_q.bind(v); }
        if let Some(v) = bin_id { count_q = count_q.bind(v); }

        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = product_id { data_q = data_q.bind(v); }
        if let Some(ref v) = keyword {
            let pattern = format!("%{v}%");
            data_q = data_q.bind(pattern);
        }
        if let Some(v) = warehouse_id { data_q = data_q.bind(v); }
        if let Some(v) = bin_id { data_q = data_q.bind(v); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<InventoryDetailView> = rows
            .iter()
            .filter_map(|r| InventoryDetailView::from_row(r).ok())
            .collect();

        let total_pages = (total as u64).div_ceil(page_size as u64) as u32;

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn list_low_stock(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<InventoryDetailView>> {
        let rows = sqlx::query(
            r#"
            SELECT sl.id as stock_ledger_id, sl.product_id, p.pdt_name as product_name,
                   p.product_code, sl.warehouse_id, w.name as warehouse_name,
                   sl.bin_id, b.code as bin_code, sl.quantity, sl.safety_stock
            FROM stock_ledger sl
            JOIN products p ON sl.product_id = p.product_id
            JOIN bins b ON sl.bin_id = b.id
            JOIN zones z ON b.zone_id = z.id
            JOIN warehouses w ON z.warehouse_id = w.id
            WHERE sl.quantity < sl.safety_stock AND sl.safety_stock > 0
            ORDER BY sl.updated_at DESC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| InventoryDetailView::from_row(r).ok())
            .collect())
    }

    // ── 事务日志 JOIN 查询 ──

    pub async fn query_transaction_details(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &TransactionLogFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<TransactionDetailView>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if filter.product_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.product_id = ${param_idx}"));
        }
        if filter.product_name.is_some() {
            param_idx += 1;
            conditions.push(format!("p.pdt_name ILIKE ${param_idx}"));
        }
        if filter.product_code.is_some() {
            param_idx += 1;
            conditions.push(format!("p.product_code ILIKE ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.warehouse_id = ${param_idx}"));
        }
        if filter.bin_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.bin_id = ${param_idx}"));
        }
        if filter.transaction_type.is_some() {
            param_idx += 1;
            conditions.push(format!("t.source_type = ${param_idx}"));
        }
        if filter.start_date.is_some() {
            param_idx += 1;
            conditions.push(format!("t.created_at >= ${param_idx}"));
        }
        if filter.end_date.is_some() {
            param_idx += 1;
            conditions.push(format!("t.created_at <= ${param_idx}"));
        }

        let where_sql = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let select_clause = "t.id, t.product_id, p.pdt_name as product_name, p.product_code, \
             t.warehouse_id, w.name as warehouse_name, \
             t.bin_id as bin_id, COALESCE(b.code, '') as bin_code, \
             t.transaction_type, t.quantity, t.source_type, t.source_id, \
             t.remark, t.operator_id, COALESCE(u.username, '') as operator_name, t.created_at";

        let from_clause = "inventory_transactions t \
             JOIN products p ON t.product_id = p.product_id \
             JOIN warehouses w ON t.warehouse_id = w.id \
             LEFT JOIN bins b ON t.bin_id = b.id \
             LEFT JOIN users u ON t.operator_id = u.user_id";

        let count_sql = format!("SELECT COUNT(*) as total FROM {from_clause} WHERE 1=1 {where_sql}");
        let data_sql = format!(
            "SELECT {select_clause} FROM {from_clause} WHERE 1=1 {where_sql} \
             ORDER BY t.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));

        // Build count query
        if let Some(v) = filter.product_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.product_name {
            count_q = count_q.bind(format!("%{v}%"));
        }
        if let Some(ref v) = filter.product_code {
            count_q = count_q.bind(format!("%{v}%"));
        }
        if let Some(v) = filter.warehouse_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.bin_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.transaction_type { count_q = count_q.bind(v); }
        if let Some(v) = filter.start_date { count_q = count_q.bind(v); }
        if let Some(v) = filter.end_date { count_q = count_q.bind(v); }

        // Build data query
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = filter.product_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.product_name {
            data_q = data_q.bind(format!("%{v}%"));
        }
        if let Some(ref v) = filter.product_code {
            data_q = data_q.bind(format!("%{v}%"));
        }
        if let Some(v) = filter.warehouse_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.bin_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.transaction_type { data_q = data_q.bind(v); }
        if let Some(v) = filter.start_date { data_q = data_q.bind(v); }
        if let Some(v) = filter.end_date { data_q = data_q.bind(v); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<TransactionDetailView> = rows
            .iter()
            .filter_map(|r| TransactionDetailView::from_row(r).ok())
            .collect();

        let total_pages = (total as u64).div_ceil(page_size as u64) as u32;

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn list_txn_details_by_product(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        Self::list_txn_details_by_filter(executor, Some(product_id), None, None, None).await
    }

    pub async fn list_txn_details_by_bin(
        executor: &mut sqlx::postgres::PgConnection,
        bin_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        Self::list_txn_details_by_filter(executor, None, Some(bin_id), None, None).await
    }

    pub async fn list_txn_details_by_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        Self::list_txn_details_by_filter(executor, None, None, Some(warehouse_id), None).await
    }

    async fn list_txn_details_by_filter(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: Option<i64>,
        bin_id: Option<i64>,
        warehouse_id: Option<i64>,
        source_type: Option<&str>,
    ) -> Result<Vec<TransactionDetailView>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if product_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.product_id = ${param_idx}"));
        }
        if bin_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.bin_id = ${param_idx}"));
        }
        if warehouse_id.is_some() {
            param_idx += 1;
            conditions.push(format!("t.warehouse_id = ${param_idx}"));
        }
        if source_type.is_some() {
            param_idx += 1;
            conditions.push(format!("t.source_type = ${param_idx}"));
        }

        let where_sql = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        let sql = format!(
            r#"
            SELECT t.id, t.product_id, p.pdt_name as product_name, p.product_code,
                   t.warehouse_id, w.name as warehouse_name,
                   t.bin_id, COALESCE(b.code, '') as bin_code,
                   t.transaction_type, t.quantity, t.source_type, t.source_id,
                   t.remark, t.operator_id, COALESCE(u.username, '') as operator_name, t.created_at
            FROM inventory_transactions t
            JOIN products p ON t.product_id = p.product_id
            JOIN warehouses w ON t.warehouse_id = w.id
            LEFT JOIN bins b ON t.bin_id = b.id
            LEFT JOIN users u ON t.operator_id = u.user_id
            WHERE 1=1 {where_sql}
            ORDER BY t.created_at DESC
            "#
        );

        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        if let Some(v) = product_id { q = q.bind(v); }
        if let Some(v) = bin_id { q = q.bind(v); }
        if let Some(v) = warehouse_id { q = q.bind(v); }
        if let Some(v) = source_type { q = q.bind(v); }

        let rows = q.fetch_all(executor).await?;
        Ok(rows
            .iter()
            .filter_map(|r| TransactionDetailView::from_row(r).ok())
            .collect())
    }

    // ── Bin 解析 ──

    pub async fn resolve_bin(
        executor: &mut sqlx::postgres::PgConnection,
        bin_id: i64,
    ) -> Result<Option<(i64, i64, i64)>> {
        let row = sqlx::query(
            "SELECT b.id as bin_id, b.zone_id, z.warehouse_id \
             FROM bins b JOIN zones z ON b.zone_id = z.id \
             WHERE b.id = $1 AND b.deleted_at IS NULL",
        )
        .bind(bin_id)
        .fetch_optional(executor)
        .await?;

        Ok(row.map(|r| {
            use sqlx::Row;
            let bin_id: i64 = r.get("bin_id");
            let zone_id: i64 = r.get("zone_id");
            let warehouse_id: i64 = r.get("warehouse_id");
            (warehouse_id, zone_id, bin_id)
        }))
    }
}
