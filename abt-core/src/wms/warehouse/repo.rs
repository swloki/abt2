use sqlx::FromRow;

use super::model::{
    Bin, BinFilter, CreateBinReq, CreateWarehouseReq, CreateZoneReq, UpdateWarehouseReq,
    Warehouse, WarehouseFilter, Zone,
};
use crate::shared::types::pagination::PaginatedResult;

pub struct WarehouseRepo;

impl WarehouseRepo {
    // ── Warehouse CRUD ──────────────────────────────────────────────────

    /// INSERT 仓库记录，返回生成的实体
    pub async fn insert_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateWarehouseReq,
        operator_id: i64,
    ) -> Result<Warehouse, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO warehouses
                (code, name, warehouse_type, status, address, manager_id,
                 is_virtual, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, code, name, warehouse_type, status, address, manager_id,
                      is_virtual, remark, operator_id, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&req.code)
        .bind(&req.name)
        .bind(req.warehouse_type)
        .bind(crate::wms::enums::WarehouseStatus::Active)
        .bind(&req.address)
        .bind(req.manager_id)
        .bind(req.is_virtual)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Warehouse::from_row(&row)
    }

    /// 按 ID 查询仓库（含已软删除的记录）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<Warehouse>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, code, name, warehouse_type, status, address, manager_id,
                   is_virtual, remark, operator_id, created_at, updated_at, deleted_at
            FROM warehouses
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| Warehouse::from_row(&r)).transpose()
    }

    /// 分页查询仓库，支持按类型/状态/关键字过滤
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &WarehouseFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Warehouse>, sqlx::Error> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        if filter.warehouse_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("warehouse_type = ${param_idx}"));
        }
        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("(code ILIKE ${param_idx} OR name ILIKE ${param_idx})"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM warehouses WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, code, name, warehouse_type, status, address, manager_id, \
             is_virtual, remark, operator_id, created_at, updated_at, deleted_at \
             FROM warehouses WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.warehouse_type {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        let keyword_pattern = filter.keyword.as_ref().map(|kw| format!("%{kw}%"));
        if let Some(ref pattern) = keyword_pattern {
            count_q = count_q.bind(pattern);
            data_q = data_q.bind(pattern);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<Warehouse> = rows
            .iter()
            .filter_map(|r| Warehouse::from_row(r).ok())
            .collect();

        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page_size as u64) as u32
        };

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }

    /// 动态 UPDATE 仓库，仅 SET 提供的字段
    pub async fn update(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &UpdateWarehouseReq,
    ) -> Result<u64, sqlx::Error> {
        let mut set_clauses = Vec::new();
        let mut param_idx = 1u32;

        if req.name.is_some() {
            param_idx += 1;
            set_clauses.push(format!("name = ${param_idx}"));
        }
        if req.warehouse_type.is_some() {
            param_idx += 1;
            set_clauses.push(format!("warehouse_type = ${param_idx}"));
        }
        if req.address.is_some() {
            param_idx += 1;
            set_clauses.push(format!("address = ${param_idx}"));
        }
        if req.manager_id.is_some() {
            param_idx += 1;
            set_clauses.push(format!("manager_id = ${param_idx}"));
        }
        if req.is_virtual.is_some() {
            param_idx += 1;
            set_clauses.push(format!("is_virtual = ${param_idx}"));
        }
        if req.remark.is_some() {
            param_idx += 1;
            set_clauses.push(format!("remark = ${param_idx}"));
        }
        if req.status.is_some() {
            param_idx += 1;
            set_clauses.push(format!("status = ${param_idx}"));
        }

        if set_clauses.is_empty() {
            return Ok(0);
        }

        param_idx += 1;
        let id_idx = param_idx;
        set_clauses.push("updated_at = NOW()".to_string());

        let sql = format!(
            "UPDATE warehouses SET {} WHERE id = ${id_idx} AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut q = sqlx::query(&sql);

        if let Some(ref v) = req.name {
            q = q.bind(v);
        }
        if let Some(v) = req.warehouse_type {
            q = q.bind(v);
        }
        if let Some(ref v) = req.address {
            q = q.bind(v);
        }
        if let Some(v) = req.manager_id {
            q = q.bind(v);
        }
        if let Some(v) = req.is_virtual {
            q = q.bind(v);
        }
        if let Some(ref v) = req.remark {
            q = q.bind(v);
        }
        if let Some(v) = req.status {
            q = q.bind(v);
        }

        q = q.bind(id);

        let result = q.execute(&mut *executor).await?;
        Ok(result.rows_affected())
    }

    /// 软删除仓库
    pub async fn soft_delete(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE warehouses SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Zone CRUD ───────────────────────────────────────────────────────

    /// INSERT 库区记录
    pub async fn insert_zone(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
        req: &CreateZoneReq,
    ) -> Result<Zone, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, warehouse_id, code, name, zone_type, sort_order, remark,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(warehouse_id)
        .bind(&req.code)
        .bind(&req.name)
        .bind(req.zone_type)
        .bind(req.sort_order.unwrap_or(0))
        .bind(&req.remark)
        .fetch_one(&mut *executor)
        .await?;

        Zone::from_row(&row)
    }

    /// 查询仓库下的所有库区（不含已软删除）
    pub async fn list_zones(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
    ) -> Result<Vec<Zone>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, warehouse_id, code, name, zone_type, sort_order, remark,
                   created_at, updated_at, deleted_at
            FROM zones
            WHERE warehouse_id = $1 AND deleted_at IS NULL
            ORDER BY sort_order, created_at
            "#,
        )
        .bind(warehouse_id)
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| Zone::from_row(r).ok())
            .collect())
    }

    // ── Bin CRUD ────────────────────────────────────────────────────────

    /// INSERT 库位记录
    pub async fn insert_bin(
        executor: &mut sqlx::postgres::PgConnection,
        zone_id: i64,
        req: &CreateBinReq,
    ) -> Result<Bin, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no,
                              capacity_limit, allowed_product_types, temperature_req, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, zone_id, code, name, row_no, column_no, layer_no,
                      capacity_limit, allowed_product_types, temperature_req,
                      status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(zone_id)
        .bind(&req.code)
        .bind(&req.name)
        .bind(&req.row_no)
        .bind(&req.column_no)
        .bind(&req.layer_no)
        .bind(req.capacity_limit)
        .bind(&req.allowed_product_types)
        .bind(&req.temperature_req)
        .bind(crate::wms::enums::BinStatus::Empty)
        .fetch_one(&mut *executor)
        .await?;

        Bin::from_row(&row)
    }

    /// 分页查询库区下的库位
    pub async fn list_bins(
        executor: &mut sqlx::postgres::PgConnection,
        zone_id: i64,
        filter: &BinFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>, sqlx::Error> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["zone_id = $1".to_string(), "deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM bins WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, zone_id, code, name, row_no, column_no, layer_no, \
             capacity_limit, allowed_product_types, temperature_req, \
             status, created_at, updated_at, deleted_at \
             FROM bins WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        // zone_id always bound at position 1
        count_q = count_q.bind(zone_id);
        data_q = data_q.bind(zone_id);

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<Bin> = rows
            .iter()
            .filter_map(|r| Bin::from_row(r).ok())
            .collect();

        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page_size as u64) as u32
        };

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }
}
