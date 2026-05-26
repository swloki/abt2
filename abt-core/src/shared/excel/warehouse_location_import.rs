//! 仓库库位 Excel 导入实现
//!
//! 适配 abt_v2 的 warehouses → zones → bins 三层结构。
//! Excel 格式为：仓库编码、仓库名称、库位编码、库位名称、容量。
//! 导入时自动为每个仓库创建默认库区（zone），库位作为 bin 写入。

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use calamine::RangeDeserializerBuilder;
use serde::Deserialize;
use sqlx::{PgPool, Row};

use super::helpers::import_range_from_source;
use super::types::{ImportResult, ImportSource};

const HEADERS: [&str; 5] = ["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"];

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "仓库编码")]
    warehouse_code: String,
    #[serde(rename = "仓库名称")]
    warehouse_name: String,
    #[serde(rename = "库位编码")]
    location_code: String,
    #[serde(rename = "库位名称")]
    location_name: Option<String>,
    #[serde(rename = "容量")]
    capacity: Option<i32>,
}

struct PendingItem {
    warehouse_code: String,
    warehouse_name: String,
    location_code: String,
    location_name: Option<String>,
    capacity: Option<i32>,
    row_index: usize,
}

pub async fn import_warehouse_locations(
    pool: &PgPool,
    source: ImportSource,
) -> Result<ImportResult> {
    let mut result = ImportResult::default();
    let range = import_range_from_source(source)?;

    let iter = RangeDeserializerBuilder::with_headers(&HEADERS).from_range(&range)?;

    let mut rows: Vec<ExcelRow> = Vec::with_capacity(range.height());
    for (row_num, row_result) in iter.enumerate() {
        match row_result {
            Ok(r) => rows.push(r),
            Err(e) => {
                result.failed_count += 1;
                result.errors.push(format!("解析第 {} 行失败: {}", row_num + 1, e));
            }
        }
    }

    let mut wh_name_consistency: HashMap<String, String> = HashMap::new();
    let mut pending: Vec<PendingItem> = Vec::with_capacity(rows.len());
    let mut seen_pairs: HashSet<(String, String)> = HashSet::with_capacity(rows.len());

    for (i, row) in rows.iter().enumerate() {
        let row_index = i + 1;
        let warehouse_code = row.warehouse_code.trim().to_string();
        let location_code = row.location_code.trim().to_string();

        if warehouse_code.is_empty() {
            result.failed_count += 1;
            result.errors.push(format!("行 {}: 仓库编码不能为空", row_index));
            continue;
        }
        if location_code.is_empty() {
            result.failed_count += 1;
            result.errors.push(format!("行 {}: 库位编码不能为空", row_index));
            continue;
        }
        if row.warehouse_name.trim().is_empty() {
            result.failed_count += 1;
            result.errors.push(format!("行 {}: 仓库名称不能为空", row_index));
            continue;
        }

        if !seen_pairs.insert((warehouse_code.clone(), location_code.clone())) {
            result.errors.push(format!(
                "行 {}: 跳过重复的 (仓库编码='{}', 库位编码='{}')",
                row_index, warehouse_code, location_code
            ));
            continue;
        }

        match wh_name_consistency.get(&warehouse_code) {
            Some(existing_name) if existing_name != &row.warehouse_name => {
                result.failed_count += 1;
                result.errors.push(format!(
                    "行 {}: 仓库编码 '{}' 名称不一致: 期望 '{}', 实际 '{}'",
                    row_index, warehouse_code, existing_name, row.warehouse_name
                ));
                continue;
            }
            None => {
                wh_name_consistency.insert(warehouse_code.clone(), row.warehouse_name.clone());
            }
            _ => {}
        }

        pending.push(PendingItem {
            warehouse_code,
            warehouse_name: row.warehouse_name.clone(),
            location_code,
            location_name: row.location_name.clone(),
            capacity: row.capacity,
            row_index,
        });
    }

    let mut tx = pool.begin().await?;

    let mut wh_cache: HashMap<String, i64> = HashMap::new();
    let mut zone_cache: HashMap<i64, i64> = HashMap::new(); // warehouse_id → default zone_id

    for item in &pending {
        let warehouse_id = match wh_cache.get(&item.warehouse_code) {
            Some(&id) => id,
            None => {
                match upsert_warehouse(&mut tx, &item.warehouse_code, &item.warehouse_name).await {
                    Ok(id) => {
                        wh_cache.insert(item.warehouse_code.clone(), id);
                        id
                    }
                    Err(e) => {
                        result.failed_count += 1;
                        result.errors.push(format!("行 {}: 仓库操作失败: {}", item.row_index, e));
                        continue;
                    }
                }
            }
        };

        let zone_id = match zone_cache.get(&warehouse_id) {
            Some(&id) => id,
            None => {
                match ensure_default_zone(&mut tx, warehouse_id).await {
                    Ok(id) => {
                        zone_cache.insert(warehouse_id, id);
                        id
                    }
                    Err(e) => {
                        result.failed_count += 1;
                        result.errors.push(format!("行 {}: 创建默认库区失败: {}", item.row_index, e));
                        continue;
                    }
                }
            }
        };

        match upsert_bin(&mut tx, zone_id, &item.location_code, item.location_name.as_deref(), item.capacity).await {
            Ok(()) => {
                result.success_count += 1;
            }
            Err(e) => {
                result.failed_count += 1;
                result.errors.push(format!("行 {}: 库位操作失败: {}", item.row_index, e));
            }
        }
    }

    tx.commit().await?;

    Ok(result)
}

async fn upsert_warehouse(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    code: &str,
    name: &str,
) -> Result<i64> {
    let row = sqlx::query(
        r#"INSERT INTO warehouses (code, name, warehouse_type, status, remark, operator_id)
           VALUES ($1, $2, 'physical', 'active', '', 0)
           ON CONFLICT (code) DO UPDATE SET name = $2, updated_at = NOW()
           RETURNING id"#,
    )
    .bind(code)
    .bind(name)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.get("id"))
}

async fn ensure_default_zone(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    warehouse_id: i64,
) -> Result<i64> {
    let existing = sqlx::query(
        "SELECT id FROM zones WHERE warehouse_id = $1 AND deleted_at IS NULL ORDER BY id LIMIT 1",
    )
    .bind(warehouse_id)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = existing {
        return Ok(row.get("id"));
    }

    let row = sqlx::query(
        r#"INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
           VALUES ($1, 'default', '默认库区', 'storage', 0, 'Excel 导入自动创建')
           RETURNING id"#,
    )
    .bind(warehouse_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.get("id"))
}

async fn upsert_bin(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    zone_id: i64,
    code: &str,
    name: Option<&str>,
    capacity: Option<i32>,
) -> Result<()> {
    let existing = sqlx::query(
        "SELECT id FROM bins WHERE zone_id = $1 AND code = $2 AND deleted_at IS NULL",
    )
    .bind(zone_id)
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(_row) = existing {
        sqlx::query(
            "UPDATE bins SET name = COALESCE($1, name), capacity_limit = COALESCE($2, capacity_limit), updated_at = NOW() WHERE zone_id = $3 AND code = $4 AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(capacity.map(|c| rust_decimal::Decimal::from(c)))
        .bind(zone_id)
        .bind(code)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            r#"INSERT INTO bins (zone_id, code, name, status, capacity_limit)
               VALUES ($1, $2, COALESCE($3, $2), 'available', $4)"#,
        )
        .bind(zone_id)
        .bind(code)
        .bind(name)
        .bind(capacity.map(|c| rust_decimal::Decimal::from(c)))
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}
