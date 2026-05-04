//! 库存数据访问层
//!
//! 提供库存的数据库 CRUD 操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{
    Inventory, InventoryDetail, InventoryExportRow, InventoryLogDetail, InventoryLogQuery,
    InventoryQuery, OperationType,
};
use crate::repositories::Executor;

/// 库存数据仓库
pub struct InventoryRepo;

impl InventoryRepo {
    /// 获取或创建库存记录（带行锁）
    pub async fn get_or_create_for_update(
        executor: Executor<'_>,
        product_id: i64,
        location_id: i64,
    ) -> Result<(i64, Decimal, bool)> {
        // 尝试获取现有记录
        let existing = sqlx::query_as!(
            Inventory,
            "SELECT inventory_id, product_id, location_id, quantity, safety_stock, batch_no, created_at, updated_at
             FROM inventory WHERE product_id = $1 AND location_id = $2 FOR UPDATE",
            product_id,
            location_id
        )
        .fetch_optional(&mut *executor)
        .await?;

        match existing {
            Some(inv) => Ok((inv.inventory_id, inv.quantity, false)),
            None => {
                // 创建新记录
                let inventory_id: i64 = sqlx::query_scalar!(
                    "INSERT INTO inventory (product_id, location_id, quantity, safety_stock)
                     VALUES ($1, $2, 0, 0) RETURNING inventory_id",
                    product_id,
                    location_id
                )
                .fetch_one(&mut *executor)
                .await?;

                Ok((inventory_id, Decimal::ZERO, true))
            }
        }
    }

    /// 更新库存数量
    pub async fn update_quantity(
        executor: Executor<'_>,
        inventory_id: i64,
        new_quantity: Decimal,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE inventory SET quantity = $1, updated_at = NOW() WHERE inventory_id = $2",
            new_quantity,
            inventory_id
        )
        .execute(&mut *executor)
        .await?;

        Ok(())
    }

    /// 设置安全库存
    pub async fn set_safety_stock(
        executor: Executor<'_>,
        product_id: i64,
        location_id: i64,
        safety_stock: Decimal,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO inventory (product_id, location_id, quantity, safety_stock)
            VALUES ($1, $2, 0, $3)
            ON CONFLICT (product_id, location_id)
            DO UPDATE SET safety_stock = $3, updated_at = NOW()
            "#,
            product_id,
            location_id,
            safety_stock
        )
        .execute(&mut *executor)
        .await?;

        Ok(())
    }

    /// 记录库存变动日志
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_log(
        executor: Executor<'_>,
        inventory_id: i64,
        product_id: i64,
        location_id: i64,
        change_qty: Decimal,
        before_qty: Decimal,
        after_qty: Decimal,
        operation_type: &OperationType,
        ref_order_type: Option<&str>,
        ref_order_id: Option<&str>,
        operator: Option<&str>,
        remark: Option<&str>,
    ) -> Result<i64> {
        let log_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO inventory_log
            (inventory_id, product_id, location_id, change_qty, before_qty, after_qty,
             operation_type, ref_order_type, ref_order_id, operator, remark)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING log_id
            "#,
            inventory_id,
            product_id,
            location_id,
            change_qty,
            before_qty,
            after_qty,
            operation_type.to_string(),
            ref_order_type,
            ref_order_id,
            operator,
            remark
        )
        .fetch_one(&mut *executor)
        .await?;

        Ok(log_id)
    }

    /// 获取产品在指定库位的库存
    pub async fn get_by_product_location(
        pool: &PgPool,
        product_id: i64,
        location_id: i64,
    ) -> Result<Option<Inventory>> {
        let row = sqlx::query_as!(
            Inventory,
            "SELECT inventory_id, product_id, location_id, quantity, safety_stock, batch_no, created_at, updated_at
             FROM inventory WHERE product_id = $1 AND location_id = $2",
            product_id,
            location_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 获取产品在所有库位的库存详情
    pub async fn get_details_by_product(
        pool: &PgPool,
        product_id: i64,
    ) -> Result<Vec<InventoryDetail>> {
        let rows = sqlx::query_as::<_, InventoryDetail>(
            r#"
            SELECT i.inventory_id, i.product_id, p.pdt_name as product_name,
                   p.product_code,
                   i.location_id, l.location_code, w.warehouse_name,
                   i.quantity, i.safety_stock,
                   i.quantity < i.safety_stock as is_low_stock, i.batch_no,
                   i.updated_at
            FROM inventory i
            JOIN products p ON i.product_id = p.product_id
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            WHERE i.product_id = $1
            ORDER BY w.warehouse_name, l.location_code
            "#,
        )
        .bind(product_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 获取库位下所有产品库存详情
    pub async fn get_details_by_location(
        pool: &PgPool,
        location_id: i64,
    ) -> Result<Vec<InventoryDetail>> {
        let rows = sqlx::query_as::<_, InventoryDetail>(
            r#"
            SELECT i.inventory_id, i.product_id, p.pdt_name as product_name,
                   p.product_code,
                   i.location_id, l.location_code, w.warehouse_name,
                   i.quantity, i.safety_stock,
                   i.quantity < i.safety_stock as is_low_stock, i.batch_no,
                   i.updated_at
            FROM inventory i
            JOIN products p ON i.product_id = p.product_id
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            WHERE i.location_id = $1
            ORDER BY p.pdt_name
            "#,
        )
        .bind(location_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 获取低库存列表
    pub async fn list_low_stock(pool: &PgPool) -> Result<Vec<InventoryDetail>> {
        let rows = sqlx::query_as::<_, InventoryDetail>(
            r#"
            SELECT i.inventory_id, i.product_id, p.pdt_name as product_name,
                   p.product_code,
                   i.location_id, l.location_code, w.warehouse_name,
                   i.quantity, i.safety_stock,
                   true as is_low_stock, i.batch_no,
                   i.updated_at
            FROM inventory i
            JOIN products p ON i.product_id = p.product_id
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            WHERE i.quantity < i.safety_stock
            ORDER BY i.quantity ASC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 分页查询库存详情（支持所有条件）
    pub async fn query_details(
        pool: &PgPool,
        query: &InventoryQuery,
    ) -> Result<(Vec<InventoryDetail>, u64)> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;
        let offset = (page - 1) * page_size;

        // 构建数据查询
        let mut qb = sqlx::QueryBuilder::new(
            r#"
            SELECT i.inventory_id, i.product_id, p.pdt_name as product_name,
                   p.product_code,
                   i.location_id, l.location_code, w.warehouse_name,
                   i.quantity, i.safety_stock,
                   i.quantity < i.safety_stock as is_low_stock, i.batch_no,
                   i.updated_at
            FROM inventory i
            JOIN products p ON i.product_id = p.product_id
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            WHERE 1=1
            "#,
        );

        Self::build_query_filter(&mut qb, query);

        qb.push(" ORDER BY w.warehouse_name, l.location_code, p.pdt_name");
        qb.push(" LIMIT ").push_bind(page_size as i64);
        qb.push(" OFFSET ").push_bind(offset as i64);

        let items = qb
            .build_query_as::<InventoryDetail>()
            .fetch_all(pool)
            .await?;

        // 构建计数查询
        let mut count_qb = sqlx::QueryBuilder::new(
            "SELECT COUNT(*) FROM inventory i JOIN location l ON i.location_id = l.location_id JOIN products p ON i.product_id = p.product_id WHERE 1=1",
        );

        Self::build_query_filter(&mut count_qb, query);

        let total: i64 = count_qb.build_query_scalar().fetch_one(pool).await?;

        Ok((items, total as u64))
    }

    /// 构建查询过滤条件
    fn build_query_filter(qb: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>, query: &InventoryQuery) {
        if let Some(product_id) = query.product_id {
            qb.push(" AND i.product_id = ").push_bind(product_id);
        }
        if let Some(location_id) = query.location_id {
            qb.push(" AND i.location_id = ").push_bind(location_id);
        }
        if let Some(warehouse_id) = query.warehouse_id {
            qb.push(" AND l.warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(ref product_name) = query.product_name
            && !product_name.is_empty()
        {
            qb.push(" AND p.pdt_name ILIKE ")
                .push_bind(format!("%{}%", product_name));
        }
        if let Some(ref product_code) = query.product_code
            && !product_code.is_empty()
        {
            qb.push(" AND p.product_code ILIKE ")
                .push_bind(format!("%{}%", product_code));
        }
        if query.low_stock_only.unwrap_or(false) {
            qb.push(" AND i.quantity < i.safety_stock");
        }
        if let Some(term_id) = query.term_id {
            qb.push(" AND EXISTS (SELECT 1 FROM term_relation tr WHERE tr.product_id = p.product_id AND tr.term_id = ");
            qb.push_bind(term_id);
            qb.push(")");
        }
    }

    /// 分页查询库存变动日志详情
    pub async fn query_logs_detail(
        pool: &PgPool,
        query: &InventoryLogQuery,
    ) -> Result<(Vec<InventoryLogDetail>, u64)> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;
        let offset = (page - 1) * page_size;

        // 基础 SQL
        let base_sql = r#"
            SELECT
                il.log_id,
                il.product_id,
                p.pdt_name as product_name,
                p.product_code,
                il.location_id,
                l.location_code,
                w.warehouse_name,
                il.change_qty,
                il.before_qty,
                il.after_qty,
                il.operation_type,
                il.ref_order_type,
                il.ref_order_id,
                il.operator,
                il.remark,
                il.created_at
            FROM inventory_log il
            JOIN products p ON il.product_id = p.product_id
            JOIN location l ON il.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
        "#;

        // 查询总数 SQL
        let count_base = r#"
            SELECT COUNT(*)
            FROM inventory_log il
            JOIN products p ON il.product_id = p.product_id
            JOIN location l ON il.location_id = l.location_id
        "#;

        // 构建 WHERE 条件
        let mut where_parts: Vec<String> = Vec::new();
        let mut param_index = 1;

        if query.product_id.is_some() {
            where_parts.push(format!("il.product_id = ${}", param_index));
            param_index += 1;
        }
        if query.product_name.is_some() {
            where_parts.push(format!("p.pdt_name ILIKE ${}", param_index));
            param_index += 1;
        }
        if query.product_code.is_some() {
            where_parts.push(format!("p.product_code ILIKE ${}", param_index));
            param_index += 1;
        }
        if query.location_id.is_some() {
            where_parts.push(format!("il.location_id = ${}", param_index));
            param_index += 1;
        }
        if query.warehouse_id.is_some() {
            where_parts.push(format!("l.warehouse_id = ${}", param_index));
            param_index += 1;
        }
        if query.operation_type.is_some() {
            where_parts.push(format!("il.operation_type = ${}", param_index));
            param_index += 1;
        }
        if query.operator.is_some() {
            where_parts.push(format!("il.operator = ${}", param_index));
            param_index += 1;
        }
        if query.start_date.is_some() {
            where_parts.push(format!("il.created_at >= ${}", param_index));
            param_index += 1;
        }
        if query.end_date.is_some() {
            where_parts.push(format!("il.created_at <= ${}", param_index));
            param_index += 1;
        }

        let where_clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_parts.join(" AND "))
        };

        // 查询总数
        let count_sql = format!("{}{}", count_base, where_clause);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);

        // 绑定计数查询参数
        if let Some(v) = query.product_id {
            count_query = count_query.bind(v);
        }
        if let Some(ref v) = query.product_name {
            count_query = count_query.bind(format!("%{}%", v));
        }
        if let Some(ref v) = query.product_code {
            count_query = count_query.bind(format!("%{}%", v));
        }
        if let Some(v) = query.location_id {
            count_query = count_query.bind(v);
        }
        if let Some(v) = query.warehouse_id {
            count_query = count_query.bind(v);
        }
        if let Some(ref v) = query.operation_type {
            count_query = count_query.bind(v.to_string());
        }
        if let Some(ref v) = query.operator {
            count_query = count_query.bind(v);
        }
        if let Some(v) = query.start_date {
            count_query = count_query.bind(v);
        }
        if let Some(v) = query.end_date {
            count_query = count_query.bind(v);
        }

        let total: u64 = count_query.fetch_one(pool).await? as u64;

        // 查询数据
        let data_sql = format!(
            "{}{} ORDER BY il.created_at DESC LIMIT ${} OFFSET ${}",
            base_sql,
            where_clause,
            param_index,
            param_index + 1
        );

        let mut data_query = sqlx::query_as::<_, InventoryLogDetail>(&data_sql);

        // 绑定数据查询参数
        if let Some(v) = query.product_id {
            data_query = data_query.bind(v);
        }
        if let Some(ref v) = query.product_name {
            data_query = data_query.bind(format!("%{}%", v));
        }
        if let Some(ref v) = query.product_code {
            data_query = data_query.bind(format!("%{}%", v));
        }
        if let Some(v) = query.location_id {
            data_query = data_query.bind(v);
        }
        if let Some(v) = query.warehouse_id {
            data_query = data_query.bind(v);
        }
        if let Some(ref v) = query.operation_type {
            data_query = data_query.bind(v.to_string());
        }
        if let Some(ref v) = query.operator {
            data_query = data_query.bind(v);
        }
        if let Some(v) = query.start_date {
            data_query = data_query.bind(v);
        }
        if let Some(v) = query.end_date {
            data_query = data_query.bind(v);
        }
        data_query = data_query.bind(page_size as i64);
        data_query = data_query.bind(offset as i64);

        let items = data_query.fetch_all(pool).await?;

        Ok((items, total))
    }

    // ========================================================================
    // Excel 导出
    // ========================================================================

    /// 获取库存导出数据（带产品和库位信息）
    pub async fn list_for_export(pool: &PgPool) -> Result<Vec<InventoryExportRow>> {
        let rows = sqlx::query_as::<_, InventoryExportRow>(
            r#"
            SELECT
                p.product_id,
                p.pdt_name,
                p.product_code,
                COALESCE(p.meta->>'specification', '') as specification,
                p.unit,
                w.warehouse_name,
                l.location_code,
                i.quantity,
                i.safety_stock,
                COALESCE((SELECT price FROM product_price WHERE product_id = p.product_id ORDER BY created_at DESC LIMIT 1), 0) as price
            FROM inventory i
            JOIN products p ON i.product_id = p.product_id
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            ORDER BY w.warehouse_name, l.location_code, p.pdt_name
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
