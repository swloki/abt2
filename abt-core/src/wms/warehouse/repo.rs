use common::PgExecutor;
use sqlx::{FromRow, Row};

use super::model::{
    Bin, BinFilter, BinInventoryStats, CreateBinReq, CreateWarehouseReq, CreateZoneReq,
    UpdateBinReq, UpdateWarehouseReq, UpdateZoneReq, Warehouse, WarehouseFilter,
    WarehouseInventoryStats, Zone,
};
use crate::shared::types::pagination::PaginatedResult;

/// 导出用的仓库-库区-库位联合行
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WarehouseExportRow {
    pub warehouse_id: i64,
    pub warehouse_code: String,
    pub warehouse_name: String,
    pub zone_id: i64,
    pub zone_code: String,
    pub zone_name: String,
    pub bin_id: i64,
    pub bin_code: String,
    pub bin_name: String,
}

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
        let mut param_idx = 0u32;

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
        let mut param_idx = 0u32;

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

    /// 按 ID 查询库区
    pub async fn get_zone_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<Zone>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, warehouse_id, code, name, zone_type, sort_order, remark,
                   created_at, updated_at, deleted_at
            FROM zones
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| Zone::from_row(&r)).transpose()
    }

    /// 动态 UPDATE 库区，仅 SET 提供的字段
    pub async fn update_zone(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &UpdateZoneReq,
    ) -> Result<u64, sqlx::Error> {
        let mut set_clauses = Vec::new();
        let mut param_idx = 0u32;

        if req.name.is_some() {
            param_idx += 1;
            set_clauses.push(format!("name = ${param_idx}"));
        }
        if req.zone_type.is_some() {
            param_idx += 1;
            set_clauses.push(format!("zone_type = ${param_idx}"));
        }
        if req.sort_order.is_some() {
            param_idx += 1;
            set_clauses.push(format!("sort_order = ${param_idx}"));
        }
        if req.remark.is_some() {
            param_idx += 1;
            set_clauses.push(format!("remark = ${param_idx}"));
        }

        if set_clauses.is_empty() {
            return Ok(0);
        }

        param_idx += 1;
        let id_idx = param_idx;
        set_clauses.push("updated_at = NOW()".to_string());

        let sql = format!(
            "UPDATE zones SET {} WHERE id = ${id_idx} AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut q = sqlx::query(&sql);

        if let Some(ref v) = req.name {
            q = q.bind(v);
        }
        if let Some(v) = req.zone_type {
            q = q.bind(v);
        }
        if let Some(v) = req.sort_order {
            q = q.bind(v);
        }
        if let Some(ref v) = req.remark {
            q = q.bind(v);
        }

        q = q.bind(id);

        let result = q.execute(&mut *executor).await?;
        Ok(result.rows_affected())
    }

    /// 软删除库区
    pub async fn soft_delete_zone(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE zones SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
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
        let mut param_idx = 0u32;

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

    /// 按 ID 查询库位
    pub async fn get_bin_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<Bin>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, zone_id, code, name, row_no, column_no, layer_no,
                   capacity_limit, allowed_product_types, temperature_req,
                   status, created_at, updated_at, deleted_at
            FROM bins
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| Bin::from_row(&r)).transpose()
    }

    /// 动态 UPDATE 库位，仅 SET 提供的字段
    pub async fn update_bin(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &UpdateBinReq,
    ) -> Result<u64, sqlx::Error> {
        let mut set_clauses = Vec::new();
        let mut param_idx = 0u32;

        if req.name.is_some() {
            param_idx += 1;
            set_clauses.push(format!("name = ${param_idx}"));
        }
        if req.row_no.is_some() {
            param_idx += 1;
            set_clauses.push(format!("row_no = ${param_idx}"));
        }
        if req.column_no.is_some() {
            param_idx += 1;
            set_clauses.push(format!("column_no = ${param_idx}"));
        }
        if req.layer_no.is_some() {
            param_idx += 1;
            set_clauses.push(format!("layer_no = ${param_idx}"));
        }
        if req.capacity_limit.is_some() {
            param_idx += 1;
            set_clauses.push(format!("capacity_limit = ${param_idx}"));
        }
        if req.allowed_product_types.is_some() {
            param_idx += 1;
            set_clauses.push(format!("allowed_product_types = ${param_idx}"));
        }
        if req.temperature_req.is_some() {
            param_idx += 1;
            set_clauses.push(format!("temperature_req = ${param_idx}"));
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
            "UPDATE bins SET {} WHERE id = ${id_idx} AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut q = sqlx::query(&sql);

        if let Some(ref v) = req.name {
            q = q.bind(v);
        }
        if let Some(ref v) = req.row_no {
            q = q.bind(v);
        }
        if let Some(ref v) = req.column_no {
            q = q.bind(v);
        }
        if let Some(ref v) = req.layer_no {
            q = q.bind(v);
        }
        if let Some(v) = req.capacity_limit {
            q = q.bind(v);
        }
        if let Some(ref v) = req.allowed_product_types {
            q = q.bind(v);
        }
        if let Some(ref v) = req.temperature_req {
            q = q.bind(v);
        }
        if let Some(v) = req.status {
            q = q.bind(v);
        }

        q = q.bind(id);

        let result = q.execute(&mut *executor).await?;
        Ok(result.rows_affected())
    }

    /// 软删除库位
    pub async fn soft_delete_bin(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE bins SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Location-compat helpers (Bin 跨 zone 聚合查询) ─────────────────

    const DEFAULT_ZONE_CODE: &str = "DEFAULT";

    /// 查找仓库下的默认库区（code = "DEFAULT"）
    pub async fn find_default_zone(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
    ) -> Result<Option<Zone>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, warehouse_id, code, name, zone_type, sort_order, remark,
                   created_at, updated_at, deleted_at
            FROM zones
            WHERE warehouse_id = $1 AND code = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(warehouse_id)
        .bind(Self::DEFAULT_ZONE_CODE)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| Zone::from_row(&r)).transpose()
    }

    /// 跨 zone 查询仓库下所有 bin（分页）
    pub async fn list_bins_by_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
        keyword: Option<&str>,
        is_active: Option<bool>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>, sqlx::Error> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec![
            "z.warehouse_id = $1".to_string(),
            "b.deleted_at IS NULL".to_string(),
            "z.deleted_at IS NULL".to_string(),
        ];
        let mut param_idx = 1u32;

        if keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("(b.code ILIKE ${param_idx} OR b.name ILIKE ${param_idx})"));
        }
        if is_active.is_some() {
            param_idx += 1;
            let val = if is_active.unwrap() {
                // active = not Disabled
                format!("b.status != ${param_idx}")
            } else {
                format!("b.status = ${param_idx}")
            };
            where_clauses.push(val);
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!(
            "SELECT COUNT(*) as total FROM bins b JOIN zones z ON b.zone_id = z.id WHERE {where_sql}"
        );
        let data_sql = format!(
            "SELECT b.id, b.zone_id, b.code, b.name, b.row_no, b.column_no, b.layer_no, \
             b.capacity_limit, b.allowed_product_types, b.temperature_req, \
             b.status, b.created_at, b.updated_at, b.deleted_at \
             FROM bins b JOIN zones z ON b.zone_id = z.id WHERE {where_sql} \
             ORDER BY b.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        count_q = count_q.bind(warehouse_id);
        data_q = data_q.bind(warehouse_id);

        let keyword_pattern = keyword.as_ref().map(|kw| format!("%{kw}%"));
        if let Some(ref pattern) = keyword_pattern {
            count_q = count_q.bind(pattern);
            data_q = data_q.bind(pattern);
        }
        if is_active.is_some() {
            let disabled = crate::wms::enums::BinStatus::Disabled;
            count_q = count_q.bind(disabled);
            data_q = data_q.bind(disabled);
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

    /// 获取 bin 并关联出 warehouse_id 和 warehouse_name
    pub async fn get_bin_with_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        bin_id: i64,
    ) -> Result<Option<(Bin, i64, String)>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT b.id, b.zone_id, b.code, b.name, b.row_no, b.column_no, b.layer_no,
                   b.capacity_limit, b.allowed_product_types, b.temperature_req,
                   b.status, b.created_at, b.updated_at, b.deleted_at,
                   w.id AS warehouse_id, w.name AS warehouse_name
            FROM bins b
            JOIN zones z ON b.zone_id = z.id
            JOIN warehouses w ON z.warehouse_id = w.id
            WHERE b.id = $1 AND b.deleted_at IS NULL AND z.deleted_at IS NULL AND w.deleted_at IS NULL
            "#,
        )
        .bind(bin_id)
        .fetch_optional(&mut *executor)
        .await?;

        match row {
            Some(r) => {
                let bin = Bin::from_row(&r)?;
                let wh_id: i64 = r.try_get("warehouse_id")?;
                let wh_name: String = r.try_get("warehouse_name")?;
                Ok(Some((bin, wh_id, wh_name)))
            }
            None => Ok(None),
        }
    }

    /// 跨仓库搜索 bin，带仓库名（分页）
    pub async fn search_bins_with_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
        keyword: Option<&str>,
        is_active: Option<bool>,
        warehouse_id: Option<i64>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<super::model::BinWithWarehouse>, sqlx::Error> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec![
            "b.deleted_at IS NULL".to_string(),
            "z.deleted_at IS NULL".to_string(),
            "w.deleted_at IS NULL".to_string(),
        ];
        let mut param_idx = 0u32;

        if keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("(b.code ILIKE ${param_idx} OR b.name ILIKE ${param_idx})"));
        }
        if is_active.is_some() {
            param_idx += 1;
            if is_active.unwrap() {
                where_clauses.push(format!("b.status != ${param_idx}"));
            } else {
                where_clauses.push(format!("b.status = ${param_idx}"));
            }
        }
        if warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("z.warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!(
            "SELECT COUNT(*) as total FROM bins b \
             JOIN zones z ON b.zone_id = z.id \
             JOIN warehouses w ON z.warehouse_id = w.id \
             WHERE {where_sql}"
        );
        let data_sql = format!(
            "SELECT b.id, b.zone_id, b.code, b.name, b.row_no, b.column_no, b.layer_no, \
             b.capacity_limit, b.allowed_product_types, b.temperature_req, \
             b.status, b.created_at, b.updated_at, b.deleted_at, \
             w.id AS warehouse_id, w.name AS warehouse_name \
             FROM bins b \
             JOIN zones z ON b.zone_id = z.id \
             JOIN warehouses w ON z.warehouse_id = w.id \
             WHERE {where_sql} \
             ORDER BY b.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        let keyword_pattern = keyword.as_ref().map(|kw| format!("%{kw}%"));
        if let Some(ref pattern) = keyword_pattern {
            count_q = count_q.bind(pattern);
            data_q = data_q.bind(pattern);
        }
        if is_active.is_some() {
            let disabled = crate::wms::enums::BinStatus::Disabled;
            count_q = count_q.bind(disabled);
            data_q = data_q.bind(disabled);
        }
        if let Some(wh_id) = warehouse_id {
            count_q = count_q.bind(wh_id);
            data_q = data_q.bind(wh_id);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<super::model::BinWithWarehouse> = rows
            .iter()
            .filter_map(|r| {
                let bin = Bin::from_row(r).ok()?;
                let wh_id: i64 = r.try_get("warehouse_id").ok()?;
                let wh_name: String = r.try_get("warehouse_name").ok()?;
                Some(super::model::BinWithWarehouse {
                    bin,
                    warehouse_id: wh_id,
                    warehouse_name: wh_name,
                })
            })
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

    /// 获取所有 bin 及仓库信息（无分页）
    pub async fn list_all_bins_with_warehouse(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<super::model::BinWithWarehouse>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT b.id, b.zone_id, b.code, b.name, b.row_no, b.column_no, b.layer_no,
                   b.capacity_limit, b.allowed_product_types, b.temperature_req,
                   b.status, b.created_at, b.updated_at, b.deleted_at,
                   w.id AS warehouse_id, w.name AS warehouse_name
            FROM bins b
            JOIN zones z ON b.zone_id = z.id
            JOIN warehouses w ON z.warehouse_id = w.id
            WHERE b.deleted_at IS NULL AND z.deleted_at IS NULL AND w.deleted_at IS NULL
            ORDER BY w.id, z.id, b.id
            "#,
        )
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| {
                let bin = Bin::from_row(r).ok()?;
                let wh_id: i64 = r.try_get("warehouse_id").ok()?;
                let wh_name: String = r.try_get("warehouse_name").ok()?;
                Some(super::model::BinWithWarehouse {
                    bin,
                    warehouse_id: wh_id,
                    warehouse_name: wh_name,
                })
            })
            .collect())
    }

    // ── Excel import/export helpers ────────────────────────────────────

    /// Resolve a location code (old flat model) to (warehouse_id, zone_id, bin_id)
    /// by searching bins.code through zones to warehouses
    pub async fn resolve_location_code(
        executor: &mut sqlx::postgres::PgConnection,
        code: &str,
    ) -> Result<Option<(i64, i64, i64)>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT w.id AS warehouse_id, z.id AS zone_id, b.id AS bin_id
            FROM bins b
            JOIN zones z ON b.zone_id = z.id
            JOIN warehouses w ON z.warehouse_id = w.id
            WHERE b.code = $1
              AND b.deleted_at IS NULL
              AND z.deleted_at IS NULL
              AND w.deleted_at IS NULL
            "#,
        )
        .bind(code)
        .fetch_optional(&mut *executor)
        .await?;

        match row {
            Some(r) => {
                let wh: i64 = r.try_get("warehouse_id")?;
                let zn: i64 = r.try_get("zone_id")?;
                let bn: i64 = r.try_get("bin_id")?;
                Ok(Some((wh, zn, bn)))
            }
            None => Ok(None),
        }
    }

    /// List all warehouses / zones / bins joined for Excel export
    pub async fn list_all_for_export(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<WarehouseExportRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT w.id   AS warehouse_id,
                   w.code AS warehouse_code,
                   w.name AS warehouse_name,
                   z.id   AS zone_id,
                   z.code AS zone_code,
                   z.name AS zone_name,
                   b.id   AS bin_id,
                   b.code AS bin_code,
                   b.name AS bin_name
            FROM bins b
            JOIN zones z ON b.zone_id = z.id
            JOIN warehouses w ON z.warehouse_id = w.id
            WHERE b.deleted_at IS NULL
              AND z.deleted_at IS NULL
              AND w.deleted_at IS NULL
            ORDER BY w.id, z.id, b.id
            "#,
        )
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| WarehouseExportRow::from_row(r).ok())
            .collect())
    }

    // ─── 库存统计 ────────────────────────────────────────────

    pub async fn get_warehouse_inventory_stats(
        executor: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> anyhow::Result<Option<WarehouseInventoryStats>> {
        let row = sqlx::query_as::<_, WarehouseInventoryStats>(
            r#"
            SELECT
                w.id AS warehouse_id,
                w.name AS warehouse_name,
                COALESCE(SUM(sl.quantity), 0) AS total_quantity,
                COUNT(DISTINCT b.id) AS bin_count,
                COUNT(DISTINCT sl.product_id) AS product_count,
                COUNT(DISTINCT CASE WHEN sl.quantity < sl.safety_stock THEN sl.product_id END) AS low_stock_count
            FROM warehouses w
            LEFT JOIN zones z ON z.warehouse_id = w.id AND z.deleted_at IS NULL
            LEFT JOIN bins b ON b.zone_id = z.id AND b.deleted_at IS NULL
            LEFT JOIN stock_ledger sl ON sl.bin_id = b.id
            WHERE w.id = $1 AND w.deleted_at IS NULL
            GROUP BY w.id, w.name
            "#,
        )
        .bind(warehouse_id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn get_bin_inventory_stats(
        executor: PgExecutor<'_>,
        bin_id: i64,
    ) -> anyhow::Result<Option<BinInventoryStats>> {
        let row = sqlx::query_as::<_, BinInventoryStats>(
            r#"
            SELECT
                b.id AS bin_id,
                b.code AS bin_code,
                b.name AS bin_name,
                COALESCE(SUM(sl.quantity), 0) AS total_quantity,
                COUNT(DISTINCT sl.product_id) AS product_count,
                COUNT(DISTINCT CASE WHEN sl.quantity < sl.safety_stock THEN sl.product_id END) AS low_stock_count
            FROM bins b
            LEFT JOIN stock_ledger sl ON sl.bin_id = b.id
            WHERE b.id = $1 AND b.deleted_at IS NULL
            GROUP BY b.id, b.code, b.name
            "#,
        )
        .bind(bin_id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn list_bin_stats_by_warehouse(
        executor: PgExecutor<'_>,
        warehouse_id: i64,
        page: u32,
        page_size: u32,
    ) -> anyhow::Result<PaginatedResult<BinInventoryStats>> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM bins b
            JOIN zones z ON z.id = b.zone_id AND z.deleted_at IS NULL
            WHERE z.warehouse_id = $1 AND b.deleted_at IS NULL
            "#,
        )
        .bind(warehouse_id)
        .fetch_one(&mut *executor)
        .await?;

        let items = sqlx::query_as::<_, BinInventoryStats>(
            r#"
            SELECT
                b.id AS bin_id,
                b.code AS bin_code,
                b.name AS bin_name,
                COALESCE(SUM(sl.quantity), 0) AS total_quantity,
                COUNT(DISTINCT sl.product_id) AS product_count,
                COUNT(DISTINCT CASE WHEN sl.quantity < sl.safety_stock THEN sl.product_id END) AS low_stock_count
            FROM bins b
            JOIN zones z ON z.id = b.zone_id AND z.deleted_at IS NULL
            LEFT JOIN stock_ledger sl ON sl.bin_id = b.id
            WHERE z.warehouse_id = $1 AND b.deleted_at IS NULL
            GROUP BY b.id, b.code, b.name
            ORDER BY b.code
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(warehouse_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(executor)
        .await?;

        Ok(PaginatedResult::new(items, total as u64, page, page_size))
    }
}
