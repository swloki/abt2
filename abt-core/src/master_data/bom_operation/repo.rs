use crate::shared::types::{PgExecutor, Result};

use super::model::*;

pub struct BomOperationRepo;

impl BomOperationRepo {
    /// 按 product_code 列出全部工序行（step_order 升序）
    pub async fn list_by_product(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Vec<BomOperation>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomOperation>(
            r#"SELECT id, product_code, step_order, process_code, process_name,
                      work_center_id, standard_time, standard_cost, allowed_loss_rate,
                      is_outsourced, is_inspection_point, is_required,
                      output_product_id, remark, source_routing_id, operator_id,
                      created_at, updated_at
               FROM bom_operations
               WHERE product_code = $1
               ORDER BY step_order"#,
        )
        .bind(product_code)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 单行查找（by product_code + step_order）
    pub async fn find(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
    ) -> Result<Option<BomOperation>> {
        let row = sqlx::query_as::<sqlx::Postgres, BomOperation>(
            r#"SELECT id, product_code, step_order, process_code, process_name,
                      work_center_id, standard_time, standard_cost, allowed_loss_rate,
                      is_outsourced, is_inspection_point, is_required,
                      output_product_id, remark, source_routing_id, operator_id,
                      created_at, updated_at
               FROM bom_operations
               WHERE product_code = $1 AND step_order = $2"#,
        )
        .bind(product_code)
        .bind(step_order)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 统计行数（apply 守卫 / UI 三态防呆用）
    pub async fn count_by_product(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<i64> {
        let n = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT COUNT(*) FROM bom_operations WHERE product_code = $1",
        )
        .bind(product_code)
        .fetch_one(executor)
        .await?;
        Ok(n)
    }

    /// UPSERT（by product_code + step_order），返回 id。ON CONFLICT 走更新。
    pub async fn upsert(
        &self,
        executor: PgExecutor<'_>,
        req: &UpsertBomOperationReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO bom_operations
                 (product_code, step_order, process_code, process_name, work_center_id,
                  standard_time, standard_cost, allowed_loss_rate, is_outsourced,
                  is_inspection_point, is_required, output_product_id, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
               ON CONFLICT (product_code, step_order) DO UPDATE SET
                 process_code        = EXCLUDED.process_code,
                 process_name        = EXCLUDED.process_name,
                 work_center_id      = EXCLUDED.work_center_id,
                 standard_time       = EXCLUDED.standard_time,
                 standard_cost       = EXCLUDED.standard_cost,
                 allowed_loss_rate   = EXCLUDED.allowed_loss_rate,
                 is_outsourced       = EXCLUDED.is_outsourced,
                 is_inspection_point = EXCLUDED.is_inspection_point,
                 is_required         = EXCLUDED.is_required,
                 output_product_id   = EXCLUDED.output_product_id,
                 remark              = EXCLUDED.remark,
                 operator_id         = EXCLUDED.operator_id,
                 updated_at          = now()
               RETURNING id"#,
        )
        .bind(&req.product_code)
        .bind(req.step_order)
        .bind(&req.process_code)
        .bind(&req.process_name)
        .bind(req.work_center_id)
        .bind(req.standard_time)
        .bind(req.standard_cost)
        .bind(req.allowed_loss_rate)
        .bind(req.is_outsourced)
        .bind(req.is_inspection_point)
        .bind(req.is_required)
        .bind(req.output_product_id)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 删单行，返回受影响行数
    pub async fn delete(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
    ) -> Result<u64> {
        let res = sqlx::query(
            "DELETE FROM bom_operations WHERE product_code = $1 AND step_order = $2",
        )
        .bind(product_code)
        .bind(step_order)
        .execute(&mut *executor)
        .await?;
        Ok(res.rows_affected())
    }

    /// 整批替换：级联清 bom_step_prices（R-5）+ DELETE bom_operations + batch upsert。
    /// ★ 事务边界：调用方负责 begin/commit（传入事务 executor）。
    /// R-5 级联清理防 step_order 复用把「焊接的价」挂到「测试工序」（计件工资资产错配红线）。
    pub async fn replace_all(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        ops: &[UpsertBomOperationReq],
        operator_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM bom_step_prices WHERE product_code = $1")
            .bind(product_code)
            .execute(&mut *executor)
            .await?;
        sqlx::query("DELETE FROM bom_operations WHERE product_code = $1")
            .bind(product_code)
            .execute(&mut *executor)
            .await?;
        for op in ops {
            self.upsert(executor, op, operator_id).await?;
        }
        Ok(())
    }

    /// copy-on-write 拷贝：从 routing_steps 全字段拷到 bom_operations。
    /// 调用方保证 force 守卫（无行 or force=true 已清空）。
    /// 不搬 unit_price / output_product_id（模板 097 已 DROP，拷贝后单独维护）。
    pub async fn copy_from_routing(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        routing_id: i64,
        operator_id: i64,
    ) -> Result<u64> {
        let res = sqlx::query(
            r#"INSERT INTO bom_operations
                 (product_code, step_order, process_code, process_name, work_center_id,
                  standard_time, standard_cost, allowed_loss_rate, is_outsourced,
                  is_inspection_point, is_required, remark, source_routing_id, operator_id, created_at)
               SELECT $1, rs.step_order, rs.process_code,
                      COALESCE(lpd.name, rs.process_code),
                      rs.work_center_id, rs.standard_time, rs.standard_cost, COALESCE(rs.allowed_loss_rate, 0),
                      rs.is_outsourced, rs.is_inspection_point, rs.is_required, rs.remark,
                      $2, $3, now()
               FROM routing_steps rs
               LEFT JOIN labor_process_dicts lpd
                 ON lpd.code = rs.process_code AND lpd.deleted_at IS NULL
               WHERE rs.routing_id = $2"#,
        )
        .bind(product_code)
        .bind(routing_id)
        .bind(operator_id)
        .execute(&mut *executor)
        .await?;
        Ok(res.rows_affected())
    }

    /// 批量同步工序名（按 process_code JOIN labor_process_dicts UPDATE process_name）
    pub async fn resync_process_names(&self, executor: PgExecutor<'_>) -> Result<u64> {
        let res = sqlx::query(
            r#"UPDATE bom_operations bo
               SET process_name = COALESCE(lpd.name, bo.process_name),
                   updated_at   = now()
               FROM labor_process_dicts lpd
               WHERE lpd.code = bo.process_code AND lpd.deleted_at IS NULL"#,
        )
        .execute(&mut *executor)
        .await?;
        Ok(res.rows_affected())
    }
}
