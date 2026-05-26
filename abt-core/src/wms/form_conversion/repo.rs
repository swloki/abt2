use sqlx::FromRow;

use super::model::{
    ConversionFilter, ConversionItem, CreateConversionItemReq, CreateConversionReq,
    FormConversion,
};
use crate::shared::types::pagination::PaginatedResult;

pub struct FormConversionRepo;

impl FormConversionRepo {
    /// 插入形态转换单及其行项目
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreateConversionReq,
        operator_id: i64,
    ) -> Result<FormConversion, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO form_conversions
                (doc_number, warehouse_id, conversion_date, status, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, doc_number, warehouse_id, conversion_date, status,
                      remark, operator_id, created_at
            "#,
        )
        .bind(doc_number)
        .bind(req.warehouse_id)
        .bind(req.conversion_date)
        .bind(super::super::enums::ConversionStatus::Draft)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        let conversion = FormConversion::from_row(&row)?;

        // 插入行项目
        for item in &req.items {
            Self::insert_item(&mut *executor, conversion.id, item).await?;
        }

        Ok(conversion)
    }

    /// 插入单个行项目
    async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        conversion_id: i64,
        item: &CreateConversionItemReq,
    ) -> Result<ConversionItem, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO conversion_items
                (conversion_id, direction, product_id, quantity, unit_cost, batch_no)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, conversion_id, direction, product_id, quantity,
                      unit_cost, batch_no
            "#,
        )
        .bind(conversion_id)
        .bind(item.direction)
        .bind(item.product_id)
        .bind(item.quantity)
        .bind(item.unit_cost)
        .bind(&item.batch_no)
        .fetch_one(&mut *executor)
        .await?;

        ConversionItem::from_row(&row)
    }

    /// 根据 ID 获取形态转换单
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<FormConversion>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, warehouse_id, conversion_date, status,
                   remark, operator_id, created_at
            FROM form_conversions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| FormConversion::from_row(&r)).transpose()
    }

    /// 获取形态转换单的所有行项目
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        conversion_id: i64,
    ) -> Result<Vec<ConversionItem>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversion_id, direction, product_id, quantity,
                   unit_cost, batch_no
            FROM conversion_items
            WHERE conversion_id = $1
            ORDER BY id
            "#,
        )
        .bind(conversion_id)
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows.iter()
            .filter_map(|r| ConversionItem::from_row(r).ok())
            .collect())
    }

    /// 更新形态转换单状态
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: super::super::enums::ConversionStatus,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE form_conversions
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

    /// 分页查询形态转换单
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &ConversionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<FormConversion>, sqlx::Error> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM form_conversions WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, warehouse_id, conversion_date, status, \
             remark, operator_id, created_at \
             FROM form_conversions WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.status {
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
        let items: Vec<FormConversion> = rows
            .iter()
            .filter_map(|r| FormConversion::from_row(r).ok())
            .collect();

        Ok(PaginatedResult::new(items, total as u64, page, page_size))
    }
}
