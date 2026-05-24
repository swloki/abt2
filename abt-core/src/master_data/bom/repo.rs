use anyhow::Result;
use common::PgExecutor;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

// ── BomRepo ──────────────────────────────────────────────────────────────────

pub struct BomRepo;

impl BomRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        bom_code: &str,
        req: &CreateBomReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO boms (bom_name, bom_code, version, status, category_id, remark, operator_id)
               VALUES ($1, $2, 1, $3, $4, $5, $6)
               RETURNING bom_id"#,
        )
        .bind(&req.bom_name)
        .bind(bom_code)
        .bind(BomStatus::Draft.as_i16())
        .bind(req.category_id)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateBomReq,
        expected_version: i32,
    ) -> Result<bool> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.bom_name.is_some() {
            sets.push(format!("bom_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.category_id.is_some() {
            sets.push(format!("category_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(true);
        }

        // optimistic locking: version must match
        sets.push("version = version + 1".to_string());
        sets.push("updated_at = NOW()".to_string());

        let version_idx = param_idx;
        param_idx += 1;
        let id_idx = param_idx;

        let sql = format!(
            "UPDATE boms SET {} WHERE bom_id = ${id_idx} AND version = ${version_idx} AND deleted_at IS NULL",
            sets.join(", ")
        );

        let mut q = sqlx::query(&sql);
        if let Some(ref v) = req.bom_name { q = q.bind(v); }
        if let Some(v) = req.category_id { q = q.bind(v); }
        if let Some(ref v) = req.remark { q = q.bind(v); }
        q = q.bind(expected_version).bind(id);

        let result = q.execute(executor).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_status(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        status: BomStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE boms SET status = $1, updated_at = NOW() WHERE bom_id = $2 AND deleted_at IS NULL")
            .bind(status.as_i16())
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE boms SET deleted_at = NOW() WHERE bom_id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<Bom>> {
        let bom = sqlx::query_as::<sqlx::Postgres, Bom>(
            "SELECT bom_id, bom_name, bom_code, version, status, category_id, remark, operator_id, created_at, updated_at, deleted_at FROM boms WHERE bom_id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(bom)
    }

    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &BomQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<Bom>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("bom_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else {
            None
        };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
            Some(status.as_i16())
        } else {
            None
        };

        let cat_param = if let Some(cat_id) = filter.category_id {
            param_idx += 1;
            conditions.push(format!("category_id = ${param_idx}"));
            Some(cat_id)
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM boms WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = cat_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT bom_id, bom_name, bom_code, version, status, category_id, remark, operator_id, created_at, updated_at, deleted_at FROM boms WHERE {where_clause} ORDER BY bom_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Bom>(&data_sql);
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = cat_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn check_name_unique(
        &self,
        executor: PgExecutor<'_>,
        name: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool> {
        let count: i64 = if let Some(eid) = exclude_id {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM boms WHERE bom_name = $1 AND bom_id != $2 AND deleted_at IS NULL",
            )
            .bind(name)
            .bind(eid)
            .fetch_one(executor)
            .await?
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM boms WHERE bom_name = $1 AND deleted_at IS NULL",
            )
            .bind(name)
            .fetch_one(executor)
            .await?
        };
        Ok(count == 0)
    }
}

// ── BomNodeRepo ──────────────────────────────────────────────────────────────

pub struct BomNodeRepo;

impl BomNodeRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        node: &NewBomNode,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO bom_nodes (bom_id, parent_node_id, product_id, quantity, unit, order_num, attr_overrides)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING node_id"#,
        )
        .bind(bom_id)
        .bind(node.parent_node_id)
        .bind(node.product_id)
        .bind(node.quantity)
        .bind(&node.unit)
        .bind(node.order_num)
        .bind(&node.attr_overrides)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        node_id: i64,
        req: &UpdateBomNodeReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.quantity.is_some() {
            sets.push(format!("quantity = ${param_idx}"));
            param_idx += 1;
        }
        if req.unit.is_some() {
            sets.push(format!("unit = ${param_idx}"));
            param_idx += 1;
        }
        if req.order_num.is_some() {
            sets.push(format!("order_num = ${param_idx}"));
            param_idx += 1;
        }
        if req.attr_overrides.is_some() {
            sets.push(format!("attr_overrides = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!(
            "UPDATE bom_nodes SET {} WHERE node_id = $1",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(node_id);
        if let Some(v) = req.quantity { q = q.bind(v); }
        if let Some(ref v) = req.unit { q = q.bind(v); }
        if let Some(v) = req.order_num { q = q.bind(v); }
        if let Some(ref v) = req.attr_overrides { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn update_parent(
        &self,
        executor: PgExecutor<'_>,
        node_id: i64,
        new_parent_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query("UPDATE bom_nodes SET parent_node_id = $1, updated_at = NOW() WHERE node_id = $2")
            .bind(new_parent_id)
            .bind(node_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_product(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        old_product_id: i64,
        new_product_id: i64,
    ) -> Result<Vec<i64>> {
        let rows = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "UPDATE bom_nodes SET product_id = $1, updated_at = NOW() WHERE bom_id = $2 AND product_id = $3 RETURNING node_id",
        )
        .bind(new_product_id)
        .bind(bom_id)
        .bind(old_product_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, node_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM bom_nodes WHERE node_id = $1")
            .bind(node_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, node_id: i64) -> Result<Option<BomNode>> {
        let node = sqlx::query_as::<sqlx::Postgres, BomNode>(
            "SELECT node_id, bom_id, parent_node_id, product_id, quantity, unit, order_num, attr_overrides, created_at, updated_at FROM bom_nodes WHERE node_id = $1",
        )
        .bind(node_id)
        .fetch_optional(executor)
        .await?;
        Ok(node)
    }

    pub async fn find_by_bom_id(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<Vec<BomNode>> {
        let nodes = sqlx::query_as::<sqlx::Postgres, BomNode>(
            "SELECT node_id, bom_id, parent_node_id, product_id, quantity, unit, order_num, attr_overrides, created_at, updated_at FROM bom_nodes WHERE bom_id = $1 ORDER BY order_num, node_id",
        )
        .bind(bom_id)
        .fetch_all(executor)
        .await?;
        Ok(nodes)
    }

    pub async fn find_leaf_nodes(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<Vec<BomNode>> {
        let nodes = sqlx::query_as::<sqlx::Postgres, BomNode>(
            r#"SELECT n.node_id, n.bom_id, n.parent_node_id, n.product_id, n.quantity, n.unit, n.order_num, n.attr_overrides, n.created_at, n.updated_at
               FROM bom_nodes n
               WHERE n.bom_id = $1
                 AND NOT EXISTS (SELECT 1 FROM bom_nodes c WHERE c.parent_node_id = n.node_id)
               ORDER BY n.order_num, n.node_id"#,
        )
        .bind(bom_id)
        .fetch_all(executor)
        .await?;
        Ok(nodes)
    }

    pub async fn find_max_order(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        parent_node_id: Option<i64>,
    ) -> Result<Option<i32>> {
        let max: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(order_num) FROM bom_nodes WHERE bom_id = $1 AND parent_node_id IS NOT DISTINCT FROM $2",
        )
        .bind(bom_id)
        .bind(parent_node_id)
        .fetch_one(executor)
        .await?;
        Ok(max)
    }

    pub async fn update_order_shift(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        parent_node_id: Option<i64>,
        from_order: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE bom_nodes SET order_num = order_num + 1 WHERE bom_id = $1 AND parent_node_id IS NOT DISTINCT FROM $2 AND order_num >= $3",
        )
        .bind(bom_id)
        .bind(parent_node_id)
        .bind(from_order)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn count_by_bom_id(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bom_nodes WHERE bom_id = $1",
        )
        .bind(bom_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    pub async fn count_children(&self, executor: PgExecutor<'_>, node_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bom_nodes WHERE parent_node_id = $1",
        )
        .bind(node_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }
}

// ── BomSnapshotRepo ──────────────────────────────────────────────────────────

pub struct BomSnapshotRepo;

impl BomSnapshotRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        version: i32,
        snapshot_data: &serde_json::Value,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO bom_snapshots (bom_id, version, snapshot_data) VALUES ($1, $2, $3) RETURNING snapshot_id",
        )
        .bind(bom_id)
        .bind(version)
        .bind(snapshot_data)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_by_bom_id(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>> {
        let limit_val = limit.unwrap_or(10);
        let snapshots = sqlx::query_as::<sqlx::Postgres, BomSnapshot>(
            "SELECT snapshot_id, bom_id, version, snapshot_data, created_at FROM bom_snapshots WHERE bom_id = $1 ORDER BY version DESC LIMIT $2",
        )
        .bind(bom_id)
        .bind(limit_val as i64)
        .fetch_all(executor)
        .await?;
        Ok(snapshots)
    }

    pub async fn find_by_bom_and_version(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        version: i32,
    ) -> Result<Option<BomSnapshot>> {
        let snapshot = sqlx::query_as::<sqlx::Postgres, BomSnapshot>(
            "SELECT snapshot_id, bom_id, version, snapshot_data, created_at FROM bom_snapshots WHERE bom_id = $1 AND version = $2",
        )
        .bind(bom_id)
        .bind(version)
        .fetch_optional(executor)
        .await?;
        Ok(snapshot)
    }
}

// ── BomCategoryRepo ──────────────────────────────────────────────────────────

pub struct BomCategoryRepo;

impl BomCategoryRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        req: &CreateBomCategoryReq,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO bom_categories (category_name, remark) VALUES ($1, $2) RETURNING category_id",
        )
        .bind(&req.category_name)
        .bind(&req.remark)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateBomCategoryReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.category_name.is_some() {
            sets.push(format!("category_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!(
            "UPDATE bom_categories SET {} WHERE category_id = $1",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(id);
        if let Some(ref v) = req.category_name { q = q.bind(v); }
        if let Some(ref v) = req.remark { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM bom_categories WHERE category_id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<BomCategory>> {
        let cat = sqlx::query_as::<sqlx::Postgres, BomCategory>(
            "SELECT category_id, category_name, remark, created_at, updated_at FROM bom_categories WHERE category_id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(cat)
    }

    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &BomCategoryQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<BomCategory>> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 1u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("category_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM bom_categories WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT category_id, category_name, remark, created_at, updated_at FROM bom_categories WHERE {where_clause} ORDER BY category_id LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, BomCategory>(&data_sql);
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn count_boms_by_category(&self, executor: PgExecutor<'_>, category_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM boms WHERE category_id = $1 AND deleted_at IS NULL",
        )
        .bind(category_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }
}
