use crate::shared::types::{PgExecutor, Result};

use super::model::*;

pub struct BomRoutingOutputRepo;

impl BomRoutingOutputRepo {
    /// UPSERT（by product_code + step_order），返回 id。ON CONFLICT 走更新。
    pub async fn upsert(
        &self,
        executor: PgExecutor<'_>,
        req: &UpsertBomOutputReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO bom_routing_outputs
                 (product_code, routing_id, step_order, output_product_id, unit_price, work_center_id, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               ON CONFLICT (product_code, step_order) DO UPDATE SET
                 output_product_id = EXCLUDED.output_product_id,
                 unit_price       = EXCLUDED.unit_price,
                 work_center_id   = EXCLUDED.work_center_id,
                 operator_id      = EXCLUDED.operator_id,
                 updated_at       = now()
               RETURNING id"#,
        )
        .bind(&req.product_code)
        .bind(req.routing_id)
        .bind(req.step_order)
        .bind(req.output_product_id)
        .bind(req.unit_price)
        .bind(req.work_center_id)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 按 product_code 取全部覆盖行（load_routings_from_template 等取数用）
    pub async fn find_by_product(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Vec<BomRoutingOutput>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomRoutingOutput>(
            r#"SELECT id, product_code, routing_id, step_order, output_product_id,
                      unit_price, work_center_id, operator_id, created_at, updated_at
               FROM bom_routing_outputs
               WHERE product_code = $1
               ORDER BY step_order"#,
        )
        .bind(product_code)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 删除单道工序的覆盖，返回受影响行数
    pub async fn delete(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
    ) -> Result<u64> {
        let res = sqlx::query(
            "DELETE FROM bom_routing_outputs WHERE product_code = $1 AND step_order = $2",
        )
        .bind(product_code)
        .bind(step_order)
        .execute(executor)
        .await?;
        Ok(res.rows_affected())
    }

    /// 工序步骤 + per-BOM 覆盖视图（前端编辑分区 / 详情页用）。
    /// work_center 名称暂不 JOIN（template_work_center_name / work_center_override_name 走 #[sqlx(default)] = None），
    /// 后续按需补 JOIN。
    pub async fn list_steps_with_output(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Vec<StepWithOutput>> {
        let rows = sqlx::query_as::<sqlx::Postgres, StepWithOutput>(
            r#"SELECT
                 rs.step_order,
                 rs.process_code,
                 lpd.name AS process_name,
                 rs.work_center_id AS template_work_center_id,
                 rs.standard_time,
                 rs.is_outsourced,
                 rs.is_inspection_point,
                 bro.id            AS output_id,
                 bro.output_product_id,
                 p.pdt_name        AS output_product_name,
                 bro.unit_price,
                 bro.work_center_id AS work_center_override_id
               FROM bom_routings br
               JOIN routing_steps rs ON rs.routing_id = br.routing_id
               LEFT JOIN labor_process_dicts lpd ON lpd.code = rs.process_code
               LEFT JOIN bom_routing_outputs bro
                 ON bro.product_code = br.product_code AND bro.step_order = rs.step_order
               LEFT JOIN products p ON p.product_id = bro.output_product_id
               WHERE br.product_code = $1
               ORDER BY rs.step_order"#,
        )
        .bind(product_code)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
