//! BOM 节点数据访问层
//!
//! 提供 bom_nodes 表的 CRUD 操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{BomNode, BomNodeRow, NewBomNode};
use crate::repositories::Executor;

pub struct BomNodeRepo;

impl BomNodeRepo {
    /// 插入单个节点，返回新生成的 id
    pub async fn insert(executor: Executor<'_>, node: &NewBomNode) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING id"#,
        )
        .bind(node.bom_id)
        .bind(node.product_id)
        .bind(&node.product_code)
        .bind(node.quantity)
        .bind(node.parent_id)
        .bind(node.loss_rate)
        .bind(node.order)
        .bind(&node.unit)
        .bind(&node.remark)
        .bind(&node.position)
        .bind(&node.work_center)
        .bind(&node.properties)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 按 bom_id 查询所有节点，按 order 排序
    pub async fn find_by_bom_id(pool: &PgPool, bom_id: i64) -> Result<Vec<BomNodeRow>> {
        let rows = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE bom_id = $1 ORDER BY "order""#,
        )
        .bind(bom_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 按 bom_id 查询所有节点并转换为 BomNode（便捷方法）
    pub async fn find_bom_nodes_by_bom_id(pool: &PgPool, bom_id: i64) -> Result<Vec<BomNode>> {
        let rows = Self::find_by_bom_id(pool, bom_id).await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// 批量按 bom_id 查询所有节点（用于列表场景，减少 N+1）
    pub async fn find_by_bom_ids(pool: &PgPool, bom_ids: &[i64]) -> Result<std::collections::HashMap<i64, Vec<BomNodeRow>>> {
        if bom_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE bom_id = ANY($1) ORDER BY "order""#,
        )
        .bind(bom_ids)
        .fetch_all(pool)
        .await?;

        let mut map: std::collections::HashMap<i64, Vec<BomNodeRow>> = std::collections::HashMap::new();
        for row in rows {
            map.entry(row.bom_id).or_default().push(row);
        }
        Ok(map)
    }

    /// 批量查询指定 BOM 列表中匹配指定产品的节点（带行锁）
    pub async fn find_by_bom_ids_and_product(
        executor: Executor<'_>,
        bom_ids: &[i64],
        product_id: i64,
    ) -> Result<Vec<BomNodeRow>> {
        if bom_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes
               WHERE bom_id = ANY($1) AND product_id = $2
               ORDER BY bom_id, "order"
               FOR UPDATE"#,
        )
        .bind(bom_ids)
        .bind(product_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 按 bom_id 查询所有节点（带行锁，用于事务内）
    pub async fn find_by_bom_id_for_update(executor: Executor<'_>, bom_id: i64) -> Result<Vec<BomNodeRow>> {
        let rows = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE bom_id = $1 ORDER BY "order" FOR UPDATE"#,
        )
        .bind(bom_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 按 id 查询单个节点
    pub async fn find_by_id(executor: Executor<'_>, id: i64) -> Result<Option<BomNodeRow>> {
        let row = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 更新节点字段
    pub async fn update(
        executor: Executor<'_>,
        id: i64,
        quantity: Decimal,
        loss_rate: Decimal,
        unit: Option<&str>,
        remark: Option<&str>,
        position: Option<&str>,
        work_center: Option<&str>,
        properties: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE bom_nodes SET quantity = $1, loss_rate = $2, unit = $3, remark = $4, position = $5, work_center = $6, properties = $7
               WHERE id = $8"#,
        )
        .bind(quantity)
        .bind(loss_rate)
        .bind(unit)
        .bind(remark)
        .bind(position)
        .bind(work_center)
        .bind(properties)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 更新节点的 parent_id
    pub async fn update_parent_id(executor: Executor<'_>, id: i64, parent_id: Option<i64>) -> Result<()> {
        sqlx::query(r#"UPDATE bom_nodes SET parent_id = $1 WHERE id = $2"#)
            .bind(parent_id)
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// 删除节点及其所有后代（递归 CTE）
    pub async fn delete_with_descendants(executor: Executor<'_>, id: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"WITH RECURSIVE descendants AS (
                SELECT id FROM bom_nodes WHERE id = $1
                UNION ALL
                SELECT n.id FROM bom_nodes n JOIN descendants d ON n.parent_id = d.id
            )
            DELETE FROM bom_nodes WHERE id IN (SELECT id FROM descendants)"#,
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 删除指定 BOM 的所有节点
    pub async fn delete_by_bom_id(executor: Executor<'_>, bom_id: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM bom_nodes WHERE bom_id = $1")
            .bind(bom_id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    /// 按 product_id 查询节点（用于产品使用查询）
    pub async fn find_bom_ids_by_product_id(pool: &PgPool, product_id: i64) -> Result<Vec<i64>> {
        let bom_ids: Vec<i64> = sqlx::query_scalar(
            "SELECT DISTINCT bom_id FROM bom_nodes WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_all(pool)
        .await?;
        Ok(bom_ids)
    }

    /// 查找指定 BOM 的根节点（parent_id IS NULL）
    pub async fn find_root_by_bom_id(pool: &PgPool, bom_id: i64) -> Result<Option<BomNodeRow>> {
        let row = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE bom_id = $1 AND parent_id IS NULL ORDER BY "order" LIMIT 1"#,
        )
        .bind(bom_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// 交换两个节点的 order 值
    pub async fn swap_order(executor: Executor<'_>, id1: i64, order1: i32, id2: i64, order2: i32) -> Result<()> {
        sqlx::query(r#"UPDATE bom_nodes SET "order" = $1 WHERE id = $2"#)
            .bind(order2)
            .bind(id1)
            .execute(&mut *executor)
            .await?;
        sqlx::query(r#"UPDATE bom_nodes SET "order" = $1 WHERE id = $2"#)
            .bind(order1)
            .bind(id2)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// 替换单个节点的产品（支持属性覆盖）
    pub async fn substitute_node_product(
        executor: Executor<'_>,
        node_id: i64,
        new_product_id: i64,
        new_product_code: Option<&str>,
        quantity: Decimal,
        loss_rate: Decimal,
        unit: Option<&str>,
        remark: Option<&str>,
        position: Option<&str>,
        work_center: Option<&str>,
        properties: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE bom_nodes SET product_id = $1, product_code = $2, quantity = $3, loss_rate = $4, unit = $5, remark = $6, position = $7, work_center = $8, properties = $9 WHERE id = $10",
        )
        .bind(new_product_id)
        .bind(new_product_code)
        .bind(quantity)
        .bind(loss_rate)
        .bind(unit)
        .bind(remark)
        .bind(position)
        .bind(work_center)
        .bind(properties)
        .bind(node_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 复制 BOM 的所有节点到新 BOM（用于 save_as）
    pub async fn copy_to_new_bom(executor: Executor<'_>, source_bom_id: i64, new_bom_id: i64) -> Result<()> {
        let source_nodes = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes WHERE bom_id = $1 ORDER BY "order""#,
        )
        .bind(source_bom_id)
        .fetch_all(&mut *executor)
        .await?;

        // 两阶段插入：先插入所有节点（parent_id=NULL），记录旧id→新id映射，再更新parent_id
        let mut id_map = std::collections::HashMap::new();

        for node in &source_nodes {
            let new_id: i64 = sqlx::query_scalar(
                r#"INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
                   VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10, $11)
                   RETURNING id"#,
            )
            .bind(new_bom_id)
            .bind(node.product_id)
            .bind(&node.product_code)
            .bind(node.quantity)
            .bind(node.loss_rate)
            .bind(node.order)
            .bind(&node.unit)
            .bind(&node.remark)
            .bind(&node.position)
            .bind(&node.work_center)
            .bind(&node.properties)
            .fetch_one(&mut *executor)
            .await?;

            id_map.insert(node.id, new_id);
        }

        // 更新 parent_id 映射
        for node in &source_nodes {
            if let Some(old_parent) = node.parent_id {
                if let (Some(&new_id), Some(&new_parent_id)) = (id_map.get(&node.id), id_map.get(&old_parent)) {
                    sqlx::query("UPDATE bom_nodes SET parent_id = $1 WHERE id = $2")
                        .bind(new_parent_id)
                        .bind(new_id)
                        .execute(&mut *executor)
                        .await?;
                }
            }
        }

        Ok(())
    }
}
