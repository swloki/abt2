use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub type LaborProcessRow = (String, i64, String, String, Decimal, Decimal, i32, Option<String>);

pub struct BomLaborProcessRepo;

impl BomLaborProcessRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, req: &CreateBomLaborProcessReq, operator_id: i64) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO bom_labor_processes (product_code, labor_process_dict_id, process_code, name, unit_price, quantity, sort_order, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#,
        )
        .bind(&req.product_code)
        .bind(req.labor_process_dict_id)
        .bind(&req.process_code)
        .bind(&req.name)
        .bind(req.unit_price)
        .bind(req.quantity)
        .bind(req.sort_order)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    #[allow(unused_assignments)]
    pub async fn update(&self, executor: PgExecutor<'_>, id: i64, req: &UpdateBomLaborProcessReq, operator_id: i64) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.labor_process_dict_id.is_some() { sets.push(format!("labor_process_dict_id = ${param_idx}")); param_idx += 1; }
        if req.process_code.is_some() { sets.push(format!("process_code = ${param_idx}")); param_idx += 1; }
        if req.name.is_some() { sets.push(format!("name = ${param_idx}")); param_idx += 1; }
        if req.unit_price.is_some() { sets.push(format!("unit_price = ${param_idx}")); param_idx += 1; }
        if req.quantity.is_some() { sets.push(format!("quantity = ${param_idx}")); param_idx += 1; }
        if req.sort_order.is_some() { sets.push(format!("sort_order = ${param_idx}")); param_idx += 1; }
        if req.remark.is_some() { sets.push(format!("remark = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        sets.push(format!("operator_id = ${param_idx}"));
        let sql = format!("UPDATE bom_labor_processes SET {} WHERE id = $1 AND deleted_at IS NULL", sets.join(", "));
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(v) = req.labor_process_dict_id { q = q.bind(v); }
        if let Some(ref v) = req.process_code { q = q.bind(v); }
        if let Some(ref v) = req.name { q = q.bind(v); }
        if let Some(v) = req.unit_price { q = q.bind(v); }
        if let Some(v) = req.quantity { q = q.bind(v); }
        if let Some(v) = req.sort_order { q = q.bind(v); }
        if let Some(ref v) = req.remark { q = q.bind(v); }
        q = q.bind(operator_id);

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE bom_labor_processes SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<BomLaborProcess>> {
        let row = sqlx::query_as::<sqlx::Postgres, BomLaborProcess>(
            "SELECT id, product_code, labor_process_dict_id, process_code, name, unit_price, quantity, sort_order, remark, operator_id, created_at, updated_at, deleted_at FROM bom_labor_processes WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn query(&self, executor: PgExecutor<'_>, filter: &BomLaborProcessQuery, page: &PageParams) -> Result<PaginatedResult<BomLaborProcess>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let product_code_param = if let Some(ref pc) = filter.product_code {
            conditions.push(format!("product_code = ${param_idx}"));
            param_idx += 1;
            Some(pc.clone())
        } else {
            None
        };

        let keyword_param = if let Some(ref kw) = filter.keyword {
            conditions.push(format!("name ILIKE ${param_idx}"));
            param_idx += 1;
            Some(format!("%{kw}%"))
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM bom_labor_processes WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = product_code_param { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;

        let data_sql = format!(
            "SELECT id, product_code, labor_process_dict_id, process_code, name, unit_price, quantity, sort_order, remark, operator_id, created_at, updated_at, deleted_at FROM bom_labor_processes WHERE {where_clause} ORDER BY sort_order, id LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, BomLaborProcess>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = product_code_param { data_q = data_q.bind(v); }
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    // ---- Excel 导入/导出辅助方法 ----

    /// 按 product_code 删除（硬删除，用于导入前清空旧数据）
    pub async fn delete_by_product_code(executor: PgExecutor<'_>, product_code: &str) -> Result<()> {
        sqlx::query("DELETE FROM bom_labor_processes WHERE product_code = $1")
            .bind(product_code)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// 批量插入劳动工序（逐行 INSERT，用于 Excel 导入）
    pub async fn batch_insert(
        executor: PgExecutor<'_>,
        rows: &[LaborProcessRow],
    ) -> Result<()> {
        for (product_code, dict_id, process_code, name, unit_price, quantity, sort_order, remark) in rows {
            sqlx::query(
                r#"INSERT INTO bom_labor_processes (product_code, labor_process_dict_id, process_code, name, unit_price, quantity, sort_order, remark)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(product_code)
            .bind(dict_id)
            .bind(process_code)
            .bind(name)
            .bind(unit_price)
            .bind(quantity)
            .bind(sort_order)
            .bind(remark)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 查询没有劳动工序成本的 BOM（用于导入校验提示）
    pub async fn find_boms_without_labor_cost(executor: PgExecutor<'_>) -> Result<Vec<BomWithoutLaborCost>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomWithoutLaborCost>(
            r#"
            SELECT DISTINCT b.bom_id, b.bom_name, bn.product_code
            FROM boms b
            JOIN bom_nodes bn ON b.bom_id = bn.bom_id
            WHERE b.deleted_at IS NULL
              AND bn.product_code NOT IN (SELECT DISTINCT product_code FROM bom_labor_processes)
            ORDER BY b.bom_id
            "#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 按 product_code 查询所有未删除的劳动工序（不分页）
    pub async fn find_all_by_product_code(&self, executor: PgExecutor<'_>, product_code: &str) -> Result<Vec<BomLaborProcess>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomLaborProcess>(
            r#"SELECT id, product_code, labor_process_dict_id, process_code, name, unit_price, quantity, sort_order, remark, operator_id, created_at, updated_at, deleted_at
               FROM bom_labor_processes
               WHERE product_code = $1 AND deleted_at IS NULL
               ORDER BY sort_order, id"#,
        )
        .bind(product_code)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
