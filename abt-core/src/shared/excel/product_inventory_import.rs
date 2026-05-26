//! 产品库存/价格/安全库存导入实现
//!
//! 适配 abt_v2 schema：stock_ledger (warehouse_id, zone_id, bin_id)、
//! price_log、product_categories。

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use calamine::RangeDeserializerBuilder;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use super::helpers::{ProgressTracker, deserialize_optional_decimal, import_range_from_source};
use super::types::{ImportResult, ImportSource};
use crate::master_data::category::repo::CategoryRepo;
use crate::master_data::price::repo::PriceRepo;
use crate::master_data::product::repo::ProductRepo;
use crate::wms::stock_ledger::repo::StockLedgerRepo;
use crate::wms::warehouse::repo::WarehouseRepo;

const PRODUCT_IMPORT_HEADERS: [&str; 8] = [
    "新编码", "旧编码", "物料名称", "库位编码", "库存数量", "价格", "安全库存", "分类ID",
];

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "新编码")]
    new_code: String,
    #[serde(rename = "旧编码")]
    old_code: Option<String>,
    #[serde(rename = "物料名称")]
    product_name: Option<String>,
    #[serde(rename = "库位编码")]
    location_code: Option<String>,
    #[serde(default, rename = "库存数量", deserialize_with = "deserialize_optional_decimal")]
    quantity: Option<Decimal>,
    #[serde(default, rename = "价格", deserialize_with = "deserialize_optional_decimal")]
    price: Option<Decimal>,
    #[serde(default, rename = "安全库存", deserialize_with = "deserialize_optional_decimal")]
    safety_stock: Option<Decimal>,
    #[serde(default, rename = "分类ID")]
    category_ids: Option<String>,
}

struct PendingItem {
    product_id: i64,
    product_code: String,
    warehouse_id: Option<i64>,
    zone_id: Option<i64>,
    bin_id: Option<i64>,
    quantity: Option<Decimal>,
    price: Option<Decimal>,
    safety_stock: Option<Decimal>,
    new_name: Option<String>,
    category_ids: Vec<i64>,
    succeeded: bool,
}

pub struct ProductInventoryImporter {
    pool: PgPool,
    tracker: Arc<ProgressTracker>,
}

impl ProductInventoryImporter {
    pub fn new(pool: PgPool, tracker: Arc<ProgressTracker>) -> Self {
        Self { pool, tracker }
    }

    pub async fn import(&self, source: ImportSource) -> Result<ImportResult> {
        let mut result = ImportResult::default();
        let range = import_range_from_source(source)?;

        let iter_results =
            RangeDeserializerBuilder::with_headers(&PRODUCT_IMPORT_HEADERS).from_range(&range)?;

        let total = range.rows().count().saturating_sub(1);
        self.tracker.set_total(total);

        let mut rows: Vec<ExcelRow> = Vec::with_capacity(total);
        for row_result in iter_results {
            match row_result {
                Ok(r) => rows.push(r),
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(format!("解析 Excel 行失败: {}", e));
                }
            }
        }

        let mut all_codes: Vec<String> = Vec::new();
        for row in &rows {
            if !row.new_code.is_empty() {
                all_codes.push(row.new_code.clone());
            }
            if let Some(ref old_code) = row.old_code
                && !old_code.is_empty()
            {
                all_codes.push(old_code.clone());
            }
        }
        all_codes.sort();
        all_codes.dedup();

        let mut conn = self.pool.acquire().await?;
        let products = ProductRepo::find_by_codes(&mut conn, &all_codes).await?;
        let product_map: HashMap<String, (i64, String)> = products
            .iter()
            .filter(|p| !p.product_code.is_empty())
            .map(|p| (p.product_code.clone(), (p.product_id, p.pdt_name.clone())))
            .collect();

        // 预加载所有 location_code → (warehouse_id, zone_id, bin_id) 映射
        let all_location_codes: Vec<String> = rows
            .iter()
            .filter_map(|r| r.location_code.as_ref().filter(|s| !s.is_empty()).cloned())
            .collect();
        let mut location_map: HashMap<String, (i64, i64, i64)> = HashMap::new();
        for code in &all_location_codes {
            if let Some(loc) = WarehouseRepo::resolve_location_code(&mut *conn, code).await? {
                location_map.insert(code.clone(), loc);
            }
        }

        let mut pending_items: Vec<PendingItem> = Vec::with_capacity(rows.len());
        for row in &rows {
            let (product_id, db_name) =
                if let Some(&(id, ref name)) = product_map.get(&row.new_code) {
                    (id, name.clone())
                } else if let Some(ref old_code) = row.old_code {
                    if let Some(&(id, ref name)) = product_map.get(old_code) {
                        (id, name.clone())
                    } else {
                        result.failed_count += 1;
                        result.errors.push(format!(
                            "产品未找到: 新编码={}, 旧编码={}",
                            row.new_code,
                            row.old_code.as_deref().unwrap_or_default()
                        ));
                        continue;
                    }
                } else {
                    result.failed_count += 1;
                    result.errors.push(format!("产品未找到: 新编码={}", row.new_code));
                    continue;
                };

            let has_quantity_or_stock = row.quantity.is_some() || row.safety_stock.is_some();

            let (warehouse_id, zone_id, bin_id) = match &row.location_code {
                Some(code) if !code.is_empty() => {
                    match location_map.get(code) {
                        Some(loc) => (Some(loc.0), Some(loc.1), Some(loc.2)),
                        None => {
                            result.failed_count += 1;
                            result.errors.push(format!("库位未找到: {}", code));
                            continue;
                        }
                    }
                }
                _ => {
                    if has_quantity_or_stock {
                        result.failed_count += 1;
                        result.errors.push(format!(
                            "产品 {} 有库存数量或安全库存但未填写库位编码",
                            row.new_code
                        ));
                        continue;
                    }
                    (None, None, None)
                }
            };

            let new_name = row
                .product_name
                .as_ref()
                .filter(|n| !n.is_empty() && **n != db_name)
                .cloned();

            let category_ids: Vec<i64> = row
                .category_ids
                .as_deref()
                .unwrap_or("")
                .split(',')
                .filter_map(|s| {
                    let s = s.trim();
                    if s.is_empty() {
                        None
                    } else {
                        s.parse::<i64>().ok()
                    }
                })
                .collect();

            pending_items.push(PendingItem {
                product_id,
                product_code: row.new_code.clone(),
                warehouse_id,
                zone_id,
                bin_id,
                quantity: row.quantity,
                price: row.price,
                safety_stock: row.safety_stock,
                new_name,
                category_ids,
                succeeded: false,
            });
        }

        let mut tx = self.pool.begin().await?;

        for item in &mut pending_items {
            self.tracker.tick();

            sqlx::query("SAVEPOINT item_sp")
                .execute(&mut *tx)
                .await?;

            let mut item_failed = false;

            if let Some(ref name) = item.new_name {
                if let Err(e) = ProductRepo::update_name(&mut tx, item.product_id, name).await {
                    result.failed_count += 1;
                    result.errors.push(format!("更新产品名称失败 {}: {}", item.product_code, e));
                    item_failed = true;
                }
            }

            if !item_failed
                && let Some(price) = item.price
            {
                let price_result = PriceRepo::upsert_price(&mut tx, item.product_id, price, "Excel 批量导入更新").await;
                if let Err(e) = price_result {
                    result.failed_count += 1;
                    result.errors.push(format!("更新价格失败 {}: {}", item.product_code, e));
                    item_failed = true;
                }
            }

            if !item_failed
                && let (Some(wh_id), Some(z_id), Some(b_id)) = (item.warehouse_id, item.zone_id, item.bin_id)
            {
                if let Some(quantity) = item.quantity {
                    match StockLedgerRepo::upsert_quantity(&mut *tx, item.product_id, wh_id, z_id, b_id, quantity).await {
                        Ok(_) => {}
                        Err(e) => {
                            result.failed_count += 1;
                            result.errors.push(format!("更新库存失败: {}", e));
                            item_failed = true;
                        }
                    }
                }

                if !item_failed
                    && let Some(safety_stock) = item.safety_stock
                {
                    if let Err(e) = StockLedgerRepo::set_safety_stock(&mut *tx, item.product_id, wh_id, z_id, b_id, safety_stock).await {
                        result.failed_count += 1;
                        result.errors.push(format!("更新安全库存失败: {}", e));
                        item_failed = true;
                    }
                }
            }

            if !item_failed
                && !item.category_ids.is_empty()
            {
                let cat_result = CategoryRepo {}.sync_product_categories(&mut tx, item.product_id, &item.category_ids).await;
                if let Err(e) = cat_result {
                    result.failed_count += 1;
                    result.errors.push(format!("更新分类关联失败 {}: {}", item.product_code, e));
                    item_failed = true;
                }
            }

            if item_failed {
                sqlx::query("ROLLBACK TO SAVEPOINT item_sp")
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("RELEASE SAVEPOINT item_sp")
                    .execute(&mut *tx)
                    .await?;
                item.succeeded = true;
                result.success_count += 1;
            }
        }

        tx.commit().await?;

        Ok(result)
    }
}
