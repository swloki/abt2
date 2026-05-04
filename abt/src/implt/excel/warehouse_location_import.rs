//! 仓库库位 Excel 导入实现
//!
//! 支持从 Excel 批量导入/更新仓库和库位。
//! 使用两阶段处理：先解析验证，再在事务中执行 UPSERT。

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use calamine::RangeDeserializerBuilder;
use serde::Deserialize;
use sqlx::{PgPool, Postgres};

use crate::models::Warehouse;
use crate::repositories::{LocationRepo, WarehouseRepo};
use crate::service::{ImportResult, RowError, ImportSource};
use super::import_range_from_source;

/// Excel 行数据结构
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

/// 待处理的导入项
struct PendingItem {
    warehouse_code: String,
    warehouse_name: String,
    location_code: String,
    location_name: Option<String>,
    capacity: Option<i32>,
    row_index: usize,
}

/// 同步模式删除阈值百分比
const SYNC_DELETE_THRESHOLD_PERCENT: f64 = 20.0;

/// 执行仓库库位导入操作
///
/// # 参数
/// - `pool`: 数据库连接池
/// - `path`: Excel 文件路径
/// - `sync_mode`: 是否启用同步模式（删除导入文件中未出现的库位）
pub async fn import_warehouse_locations(
    pool: &PgPool,
    source: ImportSource,
    sync_mode: bool,
) -> Result<ImportResult> {
        let mut result = ImportResult::default();

        // ── Phase 1: 解析 Excel ────────────────────────────────────────────
        let range = import_range_from_source(source)?;

        let headers = ["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"];
        let iter = RangeDeserializerBuilder::with_headers(&headers)
            .from_range(&range)?;

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

        // ── Phase 1b: 预加载已有数据 ────────────────────────────────────────
        let warehouse_codes: Vec<String> = {
            let mut codes: Vec<String> = rows.iter().map(|r| r.warehouse_code.clone()).collect();
            codes.sort();
            codes.dedup();
            codes
        };

        let existing_warehouses = WarehouseRepo::find_by_codes(pool, &warehouse_codes).await?;
        let mut active_wh_map: HashMap<String, Warehouse> = existing_warehouses
            .into_iter()
            .map(|w| (w.warehouse_code.clone(), w))
            .collect();

        // 批量检查软删除冲突
        let missing_codes: Vec<String> = warehouse_codes
            .iter()
            .filter(|c| !active_wh_map.contains_key(*c))
            .cloned()
            .collect();
        let mut deleted_wh_codes: HashSet<String> = HashSet::new();
        if !missing_codes.is_empty() {
            let deleted = WarehouseRepo::find_deleted_by_codes(pool, &missing_codes).await?;
            for wh in deleted {
                deleted_wh_codes.insert(wh.warehouse_code.clone());
                active_wh_map.insert(wh.warehouse_code.clone(), wh);
            }
        }

        // 用于检查仓库名称一致性
        let mut wh_name_consistency: HashMap<String, String> = HashMap::new();
        let mut pending: Vec<PendingItem> = Vec::with_capacity(rows.len());
        // 用于去重 (warehouse_code, location_code)
        let mut seen_pairs: HashSet<(String, String)> = HashSet::with_capacity(rows.len());

        for (i, row) in rows.iter().enumerate() {
            let row_index = i + 1; // 1-based for user-facing messages

            // 标准化：去除编码首尾空白
            let warehouse_code = row.warehouse_code.trim().to_string();
            let location_code = row.location_code.trim().to_string();

            if warehouse_code.is_empty() {
                result.failed_count += 1;
                result.row_errors.push(RowError {
                    row_index,
                    column_name: "仓库编码".into(),
                    reason: "仓库编码不能为空".into(),
                    raw_value: None,
                });
                continue;
            }
            if location_code.is_empty() {
                result.failed_count += 1;
                result.row_errors.push(RowError {
                    row_index,
                    column_name: "库位编码".into(),
                    reason: "库位编码不能为空".into(),
                    raw_value: None,
                });
                continue;
            }
            if row.warehouse_name.trim().is_empty() {
                result.failed_count += 1;
                result.row_errors.push(RowError {
                    row_index,
                    column_name: "仓库名称".into(),
                    reason: "仓库名称不能为空".into(),
                    raw_value: None,
                });
                continue;
            }

            // 去重：跳过 (warehouse_code, location_code) 重复的行
            if !seen_pairs.insert((warehouse_code.clone(), location_code.clone())) {
                result.errors.push(format!(
                    "行 {}: 跳过重复的 (仓库编码='{}', 库位编码='{}')",
                    row_index, warehouse_code, location_code
                ));
                continue;
            }

            // 仓库名称一致性检查（使用 trim 后的编码）
            match wh_name_consistency.get(&warehouse_code) {
                Some(existing_name) if existing_name != &row.warehouse_name => {
                    result.failed_count += 1;
                    result.row_errors.push(RowError {
                        row_index,
                        column_name: "仓库名称".into(),
                        reason: format!(
                            "仓库编码 '{}' 名称不一致: 期望 '{}', 实际 '{}'",
                            warehouse_code, existing_name, row.warehouse_name
                        ),
                        raw_value: Some(row.warehouse_name.clone()),
                    });
                    continue;
                }
                None => {
                    wh_name_consistency.insert(warehouse_code.clone(), row.warehouse_name.clone());
                }
                _ => {}
            }

            if deleted_wh_codes.contains(&warehouse_code) {
                result.failed_count += 1;
                result.row_errors.push(RowError {
                    row_index,
                    column_name: "仓库编码".into(),
                    reason: format!("仓库编码 '{}' 对应的记录已被删除，无法重新导入", warehouse_code),
                    raw_value: Some(warehouse_code.clone()),
                });
                continue;
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

        // 分批检查库位软删除冲突
        let wh_id_to_code: HashMap<i64, String> = active_wh_map
            .iter()
            .map(|(code, wh)| (wh.warehouse_id, code.clone()))
            .collect();

        let mut loc_soft_delete_conflicts: HashSet<(String, String)> =
            HashSet::with_capacity(rows.len());
        let mut by_warehouse: HashMap<i64, Vec<String>> = HashMap::new();
        for item in &pending {
            let Some(wh) = active_wh_map.get(&item.warehouse_code) else { continue };
            if wh.deleted_at.is_none() {
                by_warehouse
                    .entry(wh.warehouse_id)
                    .or_default()
                    .push(item.location_code.clone());
            }
        }
        for (wh_id, loc_codes) in &by_warehouse {
            let deleted = LocationRepo::find_deleted_by_codes(pool, *wh_id, loc_codes).await?;
            if let Some(wh_code) = wh_id_to_code.get(wh_id) {
                for loc in &deleted {
                    loc_soft_delete_conflicts
                        .insert((wh_code.clone(), loc.location_code.clone()));
                }
            }
        }

        pending.retain(|item| {
            let key = (item.warehouse_code.clone(), item.location_code.clone());
            if loc_soft_delete_conflicts.contains(&key) {
                result.failed_count += 1;
                result.row_errors.push(RowError {
                    row_index: item.row_index,
                    column_name: "库位编码".into(),
                    reason: format!(
                        "库位 '{}' 在仓库 '{}' 下已被删除，无法重新导入",
                        item.location_code, item.warehouse_code
                    ),
                    raw_value: Some(item.location_code.clone()),
                });
                false
            } else {
                true
            }
        });

        // ── Phase 1e: 预加载仓库的活跃库位（避免 Phase 2 中每行一次 SELECT）───
        let mut active_locations: HashMap<(i64, String), crate::models::Location> = HashMap::new();
        let wh_ids: Vec<i64> = active_wh_map.values().map(|w| w.warehouse_id).collect();
        if !wh_ids.is_empty() {
            let locs = sqlx::query_as::<_, crate::models::Location>(
                "SELECT location_id, warehouse_id, location_code, location_name, capacity, status, created_at, deleted_at
                 FROM location WHERE warehouse_id = ANY($1) AND deleted_at IS NULL"
            )
            .bind(&wh_ids)
            .fetch_all(pool)
            .await?;
            for loc in locs {
                active_locations.insert((loc.warehouse_id, loc.location_code.clone()), loc);
            }
        }

        // ── Phase 2: 事务中执行 UPSERT ────────────────────────────────────
        let mut tx = pool.begin().await?;

        // 仓库缓存（同一仓库的多行只创建/更新一次）
        let mut wh_cache: HashMap<String, i64> = HashMap::new();

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
                            result.row_errors.push(RowError {
                                row_index: item.row_index,
                                column_name: "仓库编码".into(),
                                reason: format!("仓库操作失败: {}", e),
                                raw_value: Some(item.warehouse_code.clone()),
                            });
                            continue;
                        }
                    }
                }
            };

            match upsert_location(
                &mut tx,
                warehouse_id,
                &item.location_code,
                item.location_name.as_deref(),
                item.capacity,
                &active_locations,
            )
            .await
            {
                Ok(()) => {
                    result.success_count += 1;
                }
                Err(e) => {
                    result.failed_count += 1;
                    result.row_errors.push(RowError {
                        row_index: item.row_index,
                        column_name: "库位编码".into(),
                        reason: format!("库位操作失败: {}", e),
                        raw_value: Some(item.location_code.clone()),
                    });
                }
            }
        }

        // ── sync_mode: 软删除未在导入文件中的库位 ──────────────────────────
        if sync_mode && !pending.is_empty() {
            let mut imported_locations: HashMap<String, HashSet<String>> = HashMap::with_capacity(wh_cache.len());
            for item in &pending {
                imported_locations
                    .entry(item.warehouse_code.clone())
                    .or_default()
                    .insert(item.location_code.clone());
            }

            for (wh_code, imported_locs) in &imported_locations {
                if let Some(&warehouse_id) = wh_cache.get(wh_code) {
                    // 获取事务级 advisory lock，防止并发同步同一仓库
                    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
                        .bind(wh_code)
                        .execute(&mut *tx)
                        .await?;

                    let existing_rows = sqlx::query_as::<_, crate::models::Location>(
                        "SELECT location_id, warehouse_id, location_code, location_name, capacity, status, created_at, deleted_at
                         FROM location WHERE warehouse_id = $1 AND deleted_at IS NULL ORDER BY location_code",
                    )
                    .bind(warehouse_id)
                    .fetch_all(&mut *tx)
                    .await?;

                    let ids: Vec<i64> = existing_rows
                        .iter()
                        .filter(|loc| !imported_locs.contains(&loc.location_code))
                        .map(|loc| loc.location_id)
                        .collect();

                    let delete_count = ids.len();
                    // 使用整数运算避免浮点数精度问题
                    if delete_count > 0 && delete_count * 100 > existing_rows.len() * (SYNC_DELETE_THRESHOLD_PERCENT as usize) {
                        result.errors.push(format!(
                            "同步模式跳过仓库 '{}': 将删除 {}% 的库位 ({}/{})，超过 {:.0}% 安全上限",
                            wh_code, (delete_count as f64 / existing_rows.len() as f64 * 100.0) as u32, delete_count, existing_rows.len(),
                            SYNC_DELETE_THRESHOLD_PERCENT
                        ));
                        continue;
                    }

                    if !ids.is_empty() {
                        sqlx::query("UPDATE location SET deleted_at = NOW() WHERE location_id = ANY($1) AND deleted_at IS NULL")
                            .bind(&ids)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
            }
        }

        tx.commit().await?;

        Ok(result)
    }

// ─── 事务内辅助函数 ───────────────────────────────────────────────────────

/// UPSERT 仓库：使用 ON CONFLICT 处理并发 INSERT 冲突
async fn upsert_warehouse(
    tx: &mut sqlx::Transaction<'static, Postgres>,
    code: &str,
    name: &str,
) -> Result<i64> {
    let id: i64 = sqlx::query_scalar!(
        r#"
        INSERT INTO warehouse (warehouse_name, warehouse_code, status)
        VALUES ($1, $2, 'active')
        ON CONFLICT (warehouse_code)
        DO UPDATE SET
            warehouse_name = CASE WHEN warehouse.deleted_at IS NULL THEN $1 ELSE warehouse.warehouse_name END,
            updated_at = NOW()
        RETURNING warehouse_id
        "#,
        name,
        code
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(id)
}

/// UPSERT 库位：使用预加载数据避免 SELECT，SAVEPOINT 隔离 INSERT 错误
async fn upsert_location(
    tx: &mut sqlx::Transaction<'static, Postgres>,
    warehouse_id: i64,
    location_code: &str,
    location_name: Option<&str>,
    capacity: Option<i32>,
    active_locations: &HashMap<(i64, String), crate::models::Location>,
) -> Result<()> {
    let key = (warehouse_id, location_code.to_string());
    if let Some(loc) = active_locations.get(&key) {
        // 有变化时更新
        if loc.location_name.as_deref() != location_name || loc.capacity != capacity {
            sqlx::query!(
                "UPDATE location SET location_name = $1, capacity = $2 WHERE location_id = $3",
                location_name,
                capacity,
                loc.location_id
            )
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    } else {
        // 新建库位，使用 SAVEPOINT 隔离 INSERT 错误
        sqlx::query("SAVEPOINT loc_insert")
            .execute(&mut **tx)
            .await?;
        match sqlx::query!(
            "INSERT INTO location (warehouse_id, location_code, location_name, capacity) VALUES ($1, $2, $3, $4)",
            warehouse_id,
            location_code,
            location_name,
            capacity
        )
        .execute(&mut **tx)
        .await
        {
            Ok(_) => {
                sqlx::query("RELEASE SAVEPOINT loc_insert")
                    .execute(&mut **tx)
                    .await?;
                Ok(())
            }
            Err(e) => {
                sqlx::query("ROLLBACK TO SAVEPOINT loc_insert")
                    .execute(&mut **tx)
                    .await?;
                Err(anyhow::anyhow!("插入库位失败: {}", e))
            }
        }
    }
}
