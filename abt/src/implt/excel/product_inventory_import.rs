//! 产品库存/价格/安全库存导入实现

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use calamine::RangeDeserializerBuilder;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use crate::implt::excel::{ProgressTracker, deserialize_optional_decimal, import_range_from_source};
use crate::repositories::{LocationRepo, ProductRepo};
use crate::service::{ExcelImportService, ImportResult, ImportSource};

/// 产品导入列定义（schema-as-code，与导出共享）
pub const PRODUCT_IMPORT_HEADERS: [&str; 8] = [
    "新编码", "旧编码", "物料名称", "仓库名称", "库位名称", "库存数量", "价格", "安全库存",
];
const _: () = assert!(PRODUCT_IMPORT_HEADERS.len() == 8);

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "新编码")]
    new_code: String,
    #[serde(rename = "旧编码")]
    old_code: Option<String>,
    #[serde(rename = "物料名称")]
    product_name: Option<String>,
    #[serde(rename = "仓库名称")]
    warehouse_name: Option<String>,
    #[serde(rename = "库位名称", alias = "库位")]
    storage: Option<String>,
    #[serde(rename = "库存数量", deserialize_with = "deserialize_optional_decimal")]
    quantity: Option<Decimal>,
    #[serde(rename = "价格", deserialize_with = "deserialize_optional_decimal")]
    price: Option<Decimal>,
    #[serde(rename = "安全库存", deserialize_with = "deserialize_optional_decimal")]
    safety_stock: Option<Decimal>,
}

struct PendingItem {
    product_id: i64,
    location_id: Option<i64>,
    quantity: Option<Decimal>,
    price: Option<Decimal>,
    safety_stock: Option<Decimal>,
    new_name: Option<String>,
}

pub struct ProductInventoryImporter {
    pool: PgPool,
    tracker: Arc<ProgressTracker>,
    operator_id: Option<i64>,
}

impl ProductInventoryImporter {
    pub fn new(pool: PgPool, tracker: Arc<ProgressTracker>) -> Self {
        Self {
            pool,
            tracker,
            operator_id: None,
        }
    }

    pub fn with_operator(mut self, operator_id: i64) -> Self {
        self.operator_id = Some(operator_id);
        self
    }
}

#[async_trait]
impl ExcelImportService for ProductInventoryImporter {
    async fn import(&self, source: ImportSource) -> Result<ImportResult> {
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
            if let Some(ref old_code) = row.old_code {
                if !old_code.is_empty() {
                    all_codes.push(old_code.clone());
                }
            }
        }
        all_codes.sort();
        all_codes.dedup();

        let products = ProductRepo::find_by_codes(&self.pool, &all_codes).await?;
        let product_map: HashMap<String, (i64, String)> = products
            .iter()
            .filter(|p| !p.meta.product_code.is_empty())
            .map(|p| (p.meta.product_code.clone(), (p.product_id, p.pdt_name.clone())))
            .collect();

        let location_map = LocationRepo::list_all_with_warehouse(&self.pool).await?;

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

            let location_id = match &row.warehouse_name {
                Some(wh) if !wh.is_empty() => {
                    if let Some(ref loc_name) = row.storage {
                        if !loc_name.is_empty() {
                            location_map
                                .get(&(wh.clone(), loc_name.clone()))
                                .map(|loc| loc.location_id)
                        } else {
                            location_map
                                .iter()
                                .find(|((w, _), _)| w == wh)
                                .map(|(_, loc)| loc.location_id)
                        }
                    } else {
                        location_map
                            .iter()
                            .find(|((w, _), _)| w == wh)
                            .map(|(_, loc)| loc.location_id)
                    }
                }
                _ => None,
            };

            let new_name = row
                .product_name
                .as_ref()
                .filter(|n| !n.is_empty() && **n != db_name)
                .cloned();

            pending_items.push(PendingItem {
                product_id,
                location_id,
                quantity: row.quantity,
                price: row.price,
                safety_stock: row.safety_stock,
                new_name,
            });
        }

        let mut tx = self.pool.begin().await?;

        for item in &pending_items {
            self.tracker.tick();

            if let Some(ref name) = item.new_name {
                if let Err(e) = ProductRepo::update_name(&mut tx, item.product_id, name).await {
                    result.failed_count += 1;
                    result
                        .errors
                        .push(format!("更新产品名称失败 product_id={}: {}", item.product_id, e));
                    continue;
                }
            }

            if let Some(price) = item.price {
                if let Err(e) = update_price_batch(&mut tx, item.product_id, price).await {
                    result.failed_count += 1;
                    result
                        .errors
                        .push(format!("更新价格失败 product_id={}: {}", item.product_id, e));
                    continue;
                }
            }

            if let Some(location_id) = item.location_id {
                if let Some(quantity) = item.quantity {
                    if let Err(e) =
                        upsert_inventory_quantity(&mut tx, item.product_id, location_id, quantity)
                            .await
                    {
                        result.failed_count += 1;
                        result.errors.push(format!("更新库存失败: {}", e));
                        continue;
                    }
                }

                if let Some(safety_stock) = item.safety_stock {
                    if let Err(e) = upsert_inventory_safety_stock(
                        &mut tx,
                        item.product_id,
                        location_id,
                        safety_stock,
                    )
                    .await
                    {
                        result.failed_count += 1;
                        result.errors.push(format!("更新安全库存失败: {}", e));
                        continue;
                    }
                }
            }

            result.success_count += 1;
        }

        tx.commit().await?;

        Ok(result)
    }
}

async fn update_price_batch(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    product_id: i64,
    price: Decimal,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE products
        SET meta = jsonb_set(
            COALESCE(meta, '{}'::jsonb),
            '{price}',
            to_jsonb($2::numeric)
        )
        WHERE product_id = $1
        "#,
        product_id,
        price
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO product_price_log (product_id, old_price, new_price, remark, created_at)
        SELECT $1,
               (SELECT meta->>'price' FROM products WHERE product_id = $1)::numeric,
               $2,
               'Excel 批量导入更新',
               NOW()
        "#,
        product_id,
        price
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_inventory_quantity(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    product_id: i64,
    location_id: i64,
    quantity: Decimal,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO inventory (product_id, location_id, quantity, safety_stock)
        VALUES ($1, $2, $3, 0)
        ON CONFLICT (product_id, location_id)
        DO UPDATE SET quantity = $3, updated_at = NOW()
        "#,
        product_id,
        location_id,
        quantity
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO inventory_log (product_id, location_id, change_qty, before_qty, after_qty, operation_type, remark, created_at)
        SELECT $1, $2, $3,
               COALESCE((SELECT quantity FROM inventory WHERE product_id = $1 AND location_id = $2), 0),
               $3,
               'adjust',
               'Excel 批量盘点导入',
               NOW()
        "#,
        product_id,
        location_id,
        quantity
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_inventory_safety_stock(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    product_id: i64,
    location_id: i64,
    safety_stock: Decimal,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO inventory (product_id, location_id, quantity, safety_stock)
        VALUES ($1, $2, 0, $3)
        ON CONFLICT (product_id, location_id)
        DO UPDATE SET safety_stock = $3, updated_at = NOW()
        "#,
        product_id,
        location_id,
        safety_stock
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
