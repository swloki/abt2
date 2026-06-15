use chrono::{DateTime, Utc};
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

// ── BomRepo ──────────────────────────────────────────────────────────────────
// Bom 实体不使用 sqlx::FromRow（bom_detail 从 bom_nodes 加载），
// 通过 BomRow 中间结构做 DB → Domain 映射

/// 根节点 product_code 子查询 — bom_nodes.parent_id=0 的节点，
/// product_code 为 NULL 时回退到 products 表
const ROOT_PRODUCT_CODE_SUBQUERY: &str = "\
(SELECT COALESCE(bn.product_code, p.product_code) \
 FROM bom_nodes bn \
 LEFT JOIN products p ON p.product_id = bn.product_id \
 WHERE bn.bom_id = boms.bom_id AND bn.parent_id = 0 \
 LIMIT 1) AS product_code";

const BOM_DB_COLUMNS: &str = "bom_id, bom_name, version, status, bom_category_id, create_at AS created_at, update_at AS updated_at, published_at, created_by";

#[derive(Debug, Clone, sqlx::FromRow)]
struct BomRow {
    bom_id: i64,
    bom_name: String,
    version: i32,
    status: BomStatus,
    bom_category_id: Option<i64>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
    published_at: Option<DateTime<Utc>>,
    created_by: Option<i64>,
    product_code: Option<String>,
}

impl From<BomRow> for Bom {
    fn from(row: BomRow) -> Self {
        Bom {
            bom_id: row.bom_id,
            bom_name: row.bom_name,
            create_at: row.created_at,
            update_at: row.updated_at,
            bom_detail: BomDetail { nodes: vec![] },
            bom_category_id: row.bom_category_id,
            status: row.status,
            version: row.version,
            published_at: row.published_at,
            created_by: row.created_by,
            product_code: row.product_code,
        }
    }
}

pub struct BomRepo;

impl BomRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        req: &CreateBomReq,
        created_by: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO boms (bom_name, version, status, bom_category_id, created_by)
               VALUES ($1, 1, $2, $3, $4)
               RETURNING bom_id"#,
        )
        .bind(&req.name)
        .bind(BomStatus::Draft.as_i16())
        .bind(req.bom_category_id)
        .bind(created_by)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateBomReq,
        expected_version: i32,
    ) -> Result<bool> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.name.is_some() {
            sets.push(format!("bom_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.bom_category_id.is_some() {
            sets.push(format!("bom_category_id = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(true);
        }

        sets.push("version = version + 1".to_string());
        sets.push("update_at = NOW()".to_string());

        let version_idx = param_idx;
        param_idx += 1;
        let id_idx = param_idx;

        let sql = format!(
            "UPDATE boms SET {} WHERE bom_id = ${id_idx} AND version = ${version_idx} AND deleted_at IS NULL",
            sets.join(", ")
        );

        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        if let Some(ref v) = req.name { q = q.bind(v); }
        if let Some(v) = req.bom_category_id { q = q.bind(v); }
        q = q.bind(expected_version).bind(id);

        let result = q.execute(executor).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_status(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        status: BomStatus,
        published_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query("UPDATE boms SET status = $1, update_at = NOW(), published_at = $3 WHERE bom_id = $2 AND deleted_at IS NULL")
            .bind(status.as_i16())
            .bind(id)
            .bind(published_at)
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
        let row = sqlx::query_as::<sqlx::Postgres, BomRow>(
            sqlx::AssertSqlSafe(format!("SELECT {BOM_DB_COLUMNS}, {ROOT_PRODUCT_CODE_SUBQUERY} FROM boms WHERE bom_id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row.map(Into::into))
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &BomQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<Bom>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!(
                "(bom_name ILIKE ${param_idx} OR EXISTS(\
                    SELECT 1 FROM bom_nodes bn \
                    LEFT JOIN products p ON p.product_id = bn.product_id \
                    WHERE bn.bom_id = boms.bom_id AND bn.parent_id = 0 \
                    AND COALESCE(bn.product_code, p.product_code) ILIKE ${param_idx}\
                ))"
            ));
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

        let cat_param = if let Some(cat_id) = filter.bom_category_id {
            param_idx += 1;
            conditions.push(format!("bom_category_id = ${param_idx}"));
            Some(cat_id)
        } else {
            None
        };
        let date_from_param = if let Some(ref df) = filter.date_from {
            if !df.is_empty() {
                param_idx += 1;
                conditions.push(format!("create_at >= ${param_idx}::date"));
                Some(df.clone())
            } else {
                None
            }
        } else {
            None
        };
        let date_to_param = if let Some(ref dt) = filter.date_to {
            if !dt.is_empty() {
                param_idx += 1;
                conditions.push(format!("create_at < (${param_idx}::date + interval '1 day')"));
                Some(dt.clone())
            } else {
                None
            }
        } else {
            None
        };
        if filter.no_labor_cost {
            conditions.push(
                "NOT EXISTS(\
                    SELECT 1 FROM bom_nodes bn \
                    JOIN bom_labor_processes blp ON blp.product_code = bn.product_code AND blp.deleted_at IS NULL \
                    WHERE bn.bom_id = boms.bom_id AND bn.parent_id = 0\
                )".to_string()
            );
        }
        let where_clause = conditions.join(" AND ");
        let count_sql = format!("SELECT COUNT(*) FROM boms WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = cat_param { count_q = count_q.bind(v); }
        if let Some(ref v) = date_from_param { count_q = count_q.bind(v); }
        if let Some(ref v) = date_to_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {BOM_DB_COLUMNS}, {ROOT_PRODUCT_CODE_SUBQUERY} FROM boms WHERE {where_clause} ORDER BY bom_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, BomRow>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = cat_param { data_q = data_q.bind(v); }
        if let Some(ref v) = date_from_param { data_q = data_q.bind(v); }
        if let Some(ref v) = date_to_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let rows = data_q.fetch_all(executor).await?;
        let items: Vec<Bom> = rows.into_iter().map(Into::into).collect();

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

    pub async fn find_product_codes_with_bom(
        &self,
        executor: PgExecutor<'_>,
        product_codes: &[String],
    ) -> Result<Vec<String>> {
        if product_codes.is_empty() {
            return Ok(Vec::new());
        }
        let codes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT bn.product_code
            FROM bom_nodes bn
            JOIN boms b ON b.bom_id = bn.bom_id
            WHERE bn.product_code = ANY($1)
              AND bn.parent_id = 0
              AND b.deleted_at IS NULL
            "#,
        )
        .bind(product_codes)
        .fetch_all(executor)
        .await?;
        Ok(codes)
    }

    /// 查找产品关联的已发布 BOM（通过根节点的 product_code 匹配）
    pub async fn find_published_by_product_code(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Option<i64>> {
        let bom_id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"
            SELECT b.bom_id
            FROM boms b
            JOIN bom_nodes bn ON bn.bom_id = b.bom_id
            WHERE bn.product_code = $1
              AND bn.parent_id = 0
              AND b.status = 2
              AND b.deleted_at IS NULL
            ORDER BY b.bom_id DESC
            LIMIT 1
            "#,
        )
        .bind(product_code)
        .fetch_optional(executor)
        .await?;
        Ok(bom_id)
    }
}

// ── BomNodeRepo ──────────────────────────────────────────────────────────────

const NODE_COLUMNS: &str = "node_id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit, remark, position, work_center, properties";

pub struct BomNodeRepo;

impl BomNodeRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        node: &NewBomNode,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO bom_nodes (bom_id, parent_id, product_id, quantity, loss_rate, unit, order_num, remark, position, work_center, properties)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING node_id"#,
        )
        .bind(bom_id)
        .bind(node.parent_id)
        .bind(node.product_id)
        .bind(node.quantity)
        .bind(node.loss_rate)
        .bind(&node.unit)
        .bind(node.order)
        .bind(&node.remark)
        .bind(&node.position)
        .bind(&node.work_center)
        .bind(&node.properties)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    #[allow(unused_assignments)]
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
        if req.loss_rate.is_some() {
            sets.push(format!("loss_rate = ${param_idx}"));
            param_idx += 1;
        }
        if req.order.is_some() {
            sets.push(format!("order_num = ${param_idx}"));
            param_idx += 1;
        }
        if req.unit.is_some() {
            sets.push(format!("unit = ${param_idx}"));
            param_idx += 1;
        }
        if req.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }
        if req.position.is_some() {
            sets.push(format!("position = ${param_idx}"));
            param_idx += 1;
        }
        if req.work_center.is_some() {
            sets.push(format!("work_center = ${param_idx}"));
            param_idx += 1;
        }
        if req.properties.is_some() {
            sets.push(format!("properties = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE bom_nodes SET {} WHERE node_id = $1",
            sets.join(", ")
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(node_id);
        if let Some(v) = req.quantity { q = q.bind(v); }
        if let Some(v) = req.loss_rate { q = q.bind(v); }
        if let Some(v) = req.order { q = q.bind(v); }
        if let Some(ref v) = req.unit { q = q.bind(v); }
        if let Some(ref v) = req.remark { q = q.bind(v); }
        if let Some(ref v) = req.position { q = q.bind(v); }
        if let Some(ref v) = req.work_center { q = q.bind(v); }
        if let Some(ref v) = req.properties { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn update_parent(
        &self,
        executor: PgExecutor<'_>,
        node_id: i64,
        new_parent_id: i64,
    ) -> Result<()> {
        sqlx::query("UPDATE bom_nodes SET parent_id = $1 WHERE node_id = $2")
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
            "UPDATE bom_nodes SET product_id = $1 WHERE bom_id = $2 AND product_id = $3 RETURNING node_id",
        )
        .bind(new_product_id)
        .bind(bom_id)
        .bind(old_product_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    #[allow(unused_assignments)]
    pub async fn update_product_with_overrides(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        old_product_id: i64,
        new_product_id: i64,
        overrides: &AttributeOverrides,
    ) -> Result<Vec<i64>> {
        let mut sets = vec!["product_id = $1".to_string()];
        let mut param_idx = 4u32;

        if overrides.quantity.is_some() {
            sets.push(format!("quantity = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.loss_rate.is_some() {
            sets.push(format!("loss_rate = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.unit.is_some() {
            sets.push(format!("unit = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.position.is_some() {
            sets.push(format!("position = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.work_center.is_some() {
            sets.push(format!("work_center = ${param_idx}"));
            param_idx += 1;
        }
        if overrides.properties.is_some() {
            sets.push(format!("properties = ${param_idx}"));
            param_idx += 1;
        }

        let sql_str = format!(
            "UPDATE bom_nodes SET {} WHERE bom_id = $2 AND product_id = $3 RETURNING node_id",
            sets.join(", ")
        );
        let mut q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(sql_str))
            .bind(new_product_id)
            .bind(bom_id)
            .bind(old_product_id);
        if let Some(v) = overrides.quantity { q = q.bind(v); }
        if let Some(v) = overrides.loss_rate { q = q.bind(v); }
        if let Some(ref v) = overrides.unit { q = q.bind(v); }
        if let Some(ref v) = overrides.remark { q = q.bind(v); }
        if let Some(ref v) = overrides.position { q = q.bind(v); }
        if let Some(ref v) = overrides.work_center { q = q.bind(v); }
        if let Some(ref v) = overrides.properties { q = q.bind(v); }

        let rows = q.fetch_all(executor).await?;
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
            sqlx::AssertSqlSafe(format!("SELECT {NODE_COLUMNS} FROM bom_nodes WHERE node_id = $1")),
        )
        .bind(node_id)
        .fetch_optional(executor)
        .await?;
        Ok(node)
    }

    pub async fn find_by_bom_id(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<Vec<BomNode>> {
        let nodes = sqlx::query_as::<sqlx::Postgres, BomNode>(
            sqlx::AssertSqlSafe(format!(r#"
                WITH RECURSIVE tree AS (
                    SELECT n.*, ARRAY[n.order_num, n.node_id] AS sort_path
                    FROM bom_nodes n
                    WHERE n.bom_id = $1 AND n.parent_id = 0
                  UNION ALL
                    SELECT c.*, p.sort_path || ARRAY[c.order_num, c.node_id]
                    FROM bom_nodes c
                    JOIN tree p ON c.parent_id = p.node_id
                    WHERE c.bom_id = $1
                )
                SELECT {NODE_COLUMNS} FROM tree
                ORDER BY sort_path
            "#)),
        )
        .bind(bom_id)
        .fetch_all(executor)
        .await?;
        Ok(nodes)
    }

    pub async fn find_leaf_nodes(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<Vec<BomNode>> {
        let nodes = sqlx::query_as::<sqlx::Postgres, BomNode>(
            sqlx::AssertSqlSafe(format!(r#"SELECT {NODE_COLUMNS}
               FROM bom_nodes
               WHERE bom_id = $1
                 AND NOT EXISTS (SELECT 1 FROM bom_nodes c WHERE c.parent_id = bom_nodes.node_id)
               ORDER BY order_num, node_id"#)),
        )
        .bind(bom_id)
        .fetch_all(executor)
        .await?;
        Ok(nodes)
    }

    pub async fn find_root_node(&self, executor: PgExecutor<'_>, bom_id: i64) -> Result<Option<BomNode>> {
        let node = sqlx::query_as::<sqlx::Postgres, BomNode>(
            sqlx::AssertSqlSafe(format!("SELECT {NODE_COLUMNS} FROM bom_nodes WHERE bom_id = $1 AND parent_id = 0 LIMIT 1")),
        )
        .bind(bom_id)
        .fetch_optional(executor)
        .await?;
        Ok(node)
    }

    pub async fn find_max_order(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        parent_id: i64,
    ) -> Result<Option<i32>> {
        let max: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(order_num) FROM bom_nodes WHERE bom_id = $1 AND parent_id = $2",
        )
        .bind(bom_id)
        .bind(parent_id)
        .fetch_one(executor)
        .await?;
        Ok(max)
    }

    pub async fn update_order_shift(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        parent_id: i64,
        from_order: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE bom_nodes SET order_num = order_num + 1 WHERE bom_id = $1 AND parent_id = $2 AND order_num >= $3",
        )
        .bind(bom_id)
        .bind(parent_id)
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
            "SELECT COUNT(*) FROM bom_nodes WHERE parent_id = $1",
        )
        .bind(node_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }
}

// ── BomSnapshotRepo ──────────────────────────────────────────────────────────

const SNAPSHOT_COLUMNS: &str = "snapshot_id, bom_id, version, bom_name, bom_detail, published_at, published_by";

pub struct BomSnapshotRepo;

impl BomSnapshotRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        bom_id: i64,
        version: i32,
        bom_name: &str,
        bom_detail: &BomDetail,
        published_by: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO bom_snapshots (bom_id, version, bom_name, bom_detail, published_at, published_by) VALUES ($1, $2, $3, $4, NOW(), $5) RETURNING snapshot_id",
        )
        .bind(bom_id)
        .bind(version)
        .bind(bom_name)
        .bind(bom_detail)
        .bind(published_by)
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
            sqlx::AssertSqlSafe(format!("SELECT {SNAPSHOT_COLUMNS} FROM bom_snapshots WHERE bom_id = $1 ORDER BY version DESC LIMIT $2")),
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
            sqlx::AssertSqlSafe(format!("SELECT {SNAPSHOT_COLUMNS} FROM bom_snapshots WHERE bom_id = $1 AND version = $2")),
        )
        .bind(bom_id)
        .bind(version)
        .fetch_optional(executor)
        .await?;
        Ok(snapshot)
    }

    /// 按 snapshot_id 加载单个快照
    pub async fn find_by_snapshot_id(
        &self,
        executor: PgExecutor<'_>,
        snapshot_id: i64,
    ) -> Result<Option<BomSnapshot>> {
        let snapshot = sqlx::query_as::<sqlx::Postgres, BomSnapshot>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {SNAPSHOT_COLUMNS} FROM bom_snapshots WHERE snapshot_id = $1"
            )),
        )
        .bind(snapshot_id)
        .fetch_optional(executor)
        .await?;
        Ok(snapshot)
    }
}

// ── BomCategoryRepo ──────────────────────────────────────────────────────────
// UML v4: BomCategory 仅 3 字段 (bom_category_id, bom_category_name, created_at)

const BOM_CATEGORY_COLUMNS: &str = "bom_category_id, bom_category_name, created_at";

pub struct BomCategoryRepo;

impl BomCategoryRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        req: &CreateBomCategoryReq,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO bom_categories (bom_category_name) VALUES ($1) RETURNING bom_category_id",
        )
        .bind(&req.bom_category_name)
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
        if req.bom_category_name.is_none() {
            return Ok(());
        }

        let sql = "UPDATE bom_categories SET bom_category_name = $2 WHERE bom_category_id = $1";
        let mut q = sqlx::query(sql).bind(id);
        if let Some(ref v) = req.bom_category_name { q = q.bind(v); }
        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM bom_categories WHERE bom_category_id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<BomCategory>> {
        let cat = sqlx::query_as::<sqlx::Postgres, BomCategory>(
            sqlx::AssertSqlSafe(format!("SELECT {BOM_CATEGORY_COLUMNS} FROM bom_categories WHERE bom_category_id = $1")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(cat)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &BomCategoryQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<BomCategory>> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("bom_category_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM bom_categories WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {BOM_CATEGORY_COLUMNS} FROM bom_categories WHERE {where_clause} ORDER BY bom_category_id LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, BomCategory>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn count_boms_by_category(&self, executor: PgExecutor<'_>, bom_category_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM boms WHERE bom_category_id = $1 AND deleted_at IS NULL",
        )
        .bind(bom_category_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }
}
