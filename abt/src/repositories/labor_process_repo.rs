//! 劳务工序数据访问层
//!
//! 提供工序、工序组、工序组成员、BOM 劳务成本的数据库 CRUD 操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序仓库
pub struct LaborProcessRepo;

impl LaborProcessRepo {
    // ========================================================================
    // 工序 CRUD
    // ========================================================================

    /// 搜索工序（支持按名称模糊查询）
    pub async fn list(
        pool: &PgPool,
        page: u32,
        page_size: u32,
        keyword: Option<&str>,
    ) -> Result<Vec<LaborProcess>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<LaborProcess> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, name, unit_price, remark, created_at, updated_at FROM labor_process WHERE name ILIKE $1 ORDER BY id ASC LIMIT $2 OFFSET $3"
            )
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, unit_price, remark, created_at, updated_at FROM labor_process ORDER BY id ASC LIMIT $1 OFFSET $2"
            )
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 统计工序数量（支持按名称模糊查询）
    pub async fn count(pool: &PgPool, keyword: Option<&str>) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar("SELECT COUNT(*) FROM labor_process WHERE name ILIKE $1")
                .bind(&pattern)
                .fetch_one(pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM labor_process")
                .fetch_one(pool)
                .await?
        };
        Ok(count)
    }

    /// 创建工序
    pub async fn insert(executor: Executor<'_>, name: &str, unit_price: Decimal, remark: Option<&str>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO labor_process (name, unit_price, remark)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            name,
            unit_price,
            remark
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工序
    pub async fn update(
        executor: Executor<'_>,
        id: i64,
        name: &str,
        unit_price: Decimal,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE labor_process
            SET name = $1, unit_price = $2, remark = $3, updated_at = NOW()
            WHERE id = $4
            "#,
            name,
            unit_price,
            remark,
            id
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 获取工序当前单价
    pub async fn get_unit_price(pool: &PgPool, id: i64) -> Result<Option<Decimal>> {
        let price: Option<Decimal> = sqlx::query_scalar!(
            "SELECT unit_price FROM labor_process WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await?;
        Ok(price)
    }

    /// 检查工序是否被引用（组成员或 BOM 劳务成本）
    pub async fn is_process_referenced<'e, E>(executor: E, process_id: i64) -> Result<bool>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let exists: bool = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM labor_process_group_member WHERE process_id = $1
            ) OR EXISTS(
                SELECT 1 FROM bom_labor_cost WHERE process_id = $1
            )
            "#,
            process_id
        )
        .fetch_one(executor)
        .await?
        .unwrap_or(false);

        Ok(exists)
    }

    /// 删除工序（被引用时拒绝）
    pub async fn delete(executor: Executor<'_>, id: i64) -> Result<u64> {
        let result: sqlx::postgres::PgQueryResult = sqlx::query!("DELETE FROM labor_process WHERE id = $1", id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    /// 查询价格变更影响的 BOM 数量和 bom_labor_cost 条目数量
    pub async fn price_change_impact(pool: &PgPool, process_id: i64) -> Result<(i64, i64)> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(DISTINCT blc.bom_id)::bigint AS affected_bom_count,
                COUNT(blc.id)::bigint AS affected_item_count
            FROM bom_labor_cost blc
            WHERE blc.process_id = $1
            "#,
        )
        .bind(process_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    // ========================================================================
    // 工序组 CRUD
    // ========================================================================

    /// 搜索工序组（支持按名称模糊查询）
    pub async fn list_groups(pool: &PgPool, page: u32, page_size: u32, keyword: Option<&str>) -> Result<Vec<LaborProcessGroup>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<LaborProcessGroup> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, name, remark, created_at, updated_at FROM labor_process_group WHERE name ILIKE $1 ORDER BY id ASC LIMIT $2 OFFSET $3"
            )
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, remark, created_at, updated_at FROM labor_process_group ORDER BY id ASC LIMIT $1 OFFSET $2"
            )
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 统计工序组数量（支持按名称模糊查询）
    pub async fn count_groups(pool: &PgPool, keyword: Option<&str>) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar("SELECT COUNT(*) FROM labor_process_group WHERE name ILIKE $1")
                .bind(&pattern)
                .fetch_one(pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM labor_process_group")
                .fetch_one(pool)
                .await?
        };
        Ok(count)
    }

    /// 创建工序组
    pub async fn insert_group(
        executor: Executor<'_>,
        name: &str,
        remark: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO labor_process_group (name, remark)
            VALUES ($1, $2)
            RETURNING id
            "#,
            name,
            remark
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工序组基本信息
    pub async fn update_group(
        executor: Executor<'_>,
        id: i64,
        name: &str,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE labor_process_group
            SET name = $1, remark = $2, updated_at = NOW()
            WHERE id = $3
            "#,
            name,
            remark,
            id
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 删除工序组（CASCADE 会自动删除成员记录）
    pub async fn delete_group(executor: Executor<'_>, id: i64) -> Result<u64> {
        let result: sqlx::postgres::PgQueryResult = sqlx::query!("DELETE FROM labor_process_group WHERE id = $1", id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    /// 检查工序组是否被 BOM 引用
    pub async fn is_group_referenced_by_bom<'e, E>(executor: E, group_id: i64) -> Result<bool>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let exists: bool = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM bom WHERE process_group_id = $1)",
            group_id
        )
        .fetch_one(executor)
        .await?
        .unwrap_or(false);

        Ok(exists)
    }

    // ========================================================================
    // 工序组成员
    // ========================================================================

    /// 查询组的所有成员（按 sort_order 排序）
    pub async fn list_group_members(pool: &PgPool, group_id: i64) -> Result<Vec<LaborProcessGroupMember>> {
        let members = sqlx::query_as!(
            LaborProcessGroupMember,
            r#"
            SELECT group_id, process_id, sort_order
            FROM labor_process_group_member
            WHERE group_id = $1
            ORDER BY sort_order ASC
            "#,
            group_id
        )
        .fetch_all(pool)
        .await?;
        Ok(members)
    }

    /// 批量设置组成员（清除旧的再插入新的；空成员列表时仅清除）
    pub async fn set_group_members(
        executor: Executor<'_>,
        group_id: i64,
        members: &[(i64, i32)],
    ) -> Result<()> {
        // 清除旧成员
        sqlx::query!(
            "DELETE FROM labor_process_group_member WHERE group_id = $1",
            group_id
        )
        .execute(&mut *executor)
        .await?;

        // 插入新成员
        if !members.is_empty() {
            let mut builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
                "INSERT INTO labor_process_group_member (group_id, process_id, sort_order) "
            );
            builder.push_values(members.iter(), |mut b, (process_id, sort_order)| {
                b.push_bind(group_id);
                b.push_bind(*process_id);
                b.push_bind(*sort_order);
            });
            builder.build().execute(executor).await?;
        }

        Ok(())
    }

    // ========================================================================
    // BOM 劳务成本
    // ========================================================================

    /// 清除 BOM 的所有劳务成本记录
    pub async fn clear_bom_labor_cost(executor: Executor<'_>, bom_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM bom_labor_cost WHERE bom_id = $1", bom_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// 批量插入 BOM 劳务成本
    pub async fn batch_insert_bom_labor_cost(
        executor: Executor<'_>,
        bom_id: i64,
        items: &[(i64, Decimal, Option<Decimal>, Option<&str>)],
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            "INSERT INTO bom_labor_cost (bom_id, process_id, quantity, unit_price_snapshot, remark) "
        );
        builder.push_values(items.iter(), |mut b, (process_id, quantity, snapshot, remark)| {
            b.push_bind(bom_id);
            b.push_bind(*process_id);
            b.push_bind(*quantity);
            b.push_bind(*snapshot);
            b.push_bind(*remark);
        });
        builder.build().execute(executor).await?;
        Ok(())
    }

    /// 查询 BOM 劳务成本（含工序信息）
    pub async fn get_bom_labor_cost(pool: &PgPool, bom_id: i64) -> Result<Vec<BomLaborCostItem>> {
        let items = sqlx::query_as!(
            BomLaborCostItem,
            r#"
            SELECT
                blc.id,
                blc.process_id,
                lp.name AS process_name,
                lp.unit_price AS current_unit_price,
                blc.unit_price_snapshot AS snapshot_unit_price,
                blc.quantity,
                blc.remark
            FROM bom_labor_cost blc
            JOIN labor_process lp ON lp.id = blc.process_id
            WHERE blc.bom_id = $1
            ORDER BY blc.id ASC
            "#,
            bom_id
        )
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// 更新 BOM 的 process_group_id
    pub async fn set_bom_process_group(executor: Executor<'_>, bom_id: i64, group_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE bom SET process_group_id = $1 WHERE bom_id = $2",
            group_id,
            bom_id
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    // ========================================================================
    // 批量查询
    // ========================================================================

    /// 批量获取多个组的成员
    pub async fn list_group_members_batch(pool: &PgPool, group_ids: &[i64]) -> Result<Vec<LaborProcessGroupMember>> {
        if group_ids.is_empty() {
            return Ok(vec![]);
        }
        let members = sqlx::query_as!(
            LaborProcessGroupMember,
            r#"
            SELECT group_id, process_id, sort_order
            FROM labor_process_group_member
            WHERE group_id = ANY($1)
            ORDER BY sort_order ASC
            "#,
            group_ids
        )
        .fetch_all(pool)
        .await?;
        Ok(members)
    }

    /// 锁定工序行并获取单价（防止并发修改价格快照）
    pub async fn lock_and_get_unit_prices(
        executor: Executor<'_>,
        process_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        if process_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows: Vec<(i64, Decimal)> = sqlx::query_as(
            "SELECT id, unit_price FROM labor_process WHERE id = ANY($1) FOR UPDATE"
        )
        .bind(process_ids)
        .fetch_all(executor)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// 获取 BOM 关联的工序组及成员（一次查询，消除嵌套 Option）
    pub async fn get_bom_group_with_members(pool: &PgPool, bom_id: i64) -> Result<Option<LaborProcessGroupWithMembers>> {
        let row = sqlx::query_as!(
            LaborProcessGroup,
            r#"
            SELECT g.id, g.name, g.remark, g.created_at, g.updated_at
            FROM bom b
            JOIN labor_process_group g ON g.id = b.process_group_id
            WHERE b.bom_id = $1
            "#,
            bom_id
        )
        .fetch_optional(pool)
        .await?;

        let group = match row {
            Some(g) => g,
            None => return Ok(None),
        };

        let members = Self::list_group_members(pool, group.id).await?;
        Ok(Some(LaborProcessGroupWithMembers { group, members }))
    }

    // ========================================================================
    // Excel 导入导出
    // ========================================================================

    /// 查询所有工序（用于导出）
    pub async fn list_all(pool: &PgPool) -> Result<Vec<LaborProcess>> {
        let items = sqlx::query_as(
            "SELECT id, name, unit_price, remark, created_at, updated_at FROM labor_process ORDER BY name ASC"
        )
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// 批量 upsert 工序（ON CONFLICT by name）
    /// items: (name, unit_price, remark)
    /// 返回受影响的行数
    pub async fn batch_upsert(
        executor: Executor<'_>,
        items: &[(String, Decimal, Option<String>)],
    ) -> Result<u64> {
        if items.is_empty() {
            return Ok(0);
        }

        let mut builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            "INSERT INTO labor_process (name, unit_price, remark) "
        );
        builder.push_values(items.iter(), |mut b, (name, unit_price, remark)| {
            b.push_bind(name);
            b.push_bind(*unit_price);
            b.push_bind(remark);
        });
        builder.push(
            " ON CONFLICT (name) DO UPDATE SET unit_price = EXCLUDED.unit_price, remark = EXCLUDED.remark, updated_at = NOW()"
        );

        let result = builder.build().execute(executor).await?;
        Ok(result.rows_affected())
    }

    /// 按名称批量查询工序
    pub async fn find_by_names(pool: &PgPool, names: &[String]) -> Result<Vec<LaborProcess>> {
        if names.is_empty() {
            return Ok(vec![]);
        }
        let items = sqlx::query_as(
            "SELECT id, name, unit_price, remark, created_at, updated_at FROM labor_process WHERE name = ANY($1)"
        )
        .bind(names)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
