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
use crate::repositories::{InventoryRepo, LocationRepo, ProductRepo};
use crate::service::{ExcelImportService, ImportResult, ImportSource};

/// 产品导入列定义（schema-as-code，与导出共享）
pub const PRODUCT_IMPORT_HEADERS: [&str; 7] = [
    "新编码", "旧编码", "物料名称", "库位编码", "库存数量", "价格", "安全库存",
];
const _: () = assert!(PRODUCT_IMPORT_HEADERS.len() == 7);

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
            .filter(|p| !p.product_code.is_empty())
            .map(|p| (p.product_code.clone(), (p.product_id, p.pdt_name.clone())))
            .collect();

        let location_map = LocationRepo::list_all_by_code(&self.pool).await?;

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

            let location_id = match &row.location_code {
                Some(code) if !code.is_empty() => {
                    match location_map.get(code) {
                        Some(loc) => Some(loc.location_id),
                        None => {
                            result.failed_count += 1;
                            result.errors.push(format!("库位未找到: {}", code));
                            continue;
                        }
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

        // 验证一库位一产品约束
        let mut location_ids_to_check: Vec<i64> = pending_items
            .iter()
            .filter_map(|item| item.location_id)
            .collect();
        location_ids_to_check.sort();
        location_ids_to_check.dedup();

        let db_occupants = InventoryRepo::get_location_occupants(&self.pool, &location_ids_to_check).await?;

        // 检查导入数据内部冲突 + 与数据库冲突
        let mut import_location_product: HashMap<i64, i64> = HashMap::new();
        let mut conflict_set: std::collections::HashSet<i64> = std::collections::HashSet::new();

        for item in &pending_items {
            if let Some(loc_id) = item.location_id {
                // 检查与数据库已有数据冲突
                if let Some(db_products) = db_occupants.get(&loc_id) {
                    if !db_products.iter().any(|&pid| pid == item.product_id) {
                        conflict_set.insert(loc_id);
                    }
                }

                // 检查导入数据内部冲突
                if let Some(&existing_pid) = import_location_product.get(&loc_id) {
                    if existing_pid != item.product_id {
                        conflict_set.insert(loc_id);
                    }
                } else {
                    import_location_product.insert(loc_id, item.product_id);
                }
            }
        }

        if !conflict_set.is_empty() {
            // 将冲突的 location_id 反查 location_code 用于错误提示
            let conflict_codes: Vec<String> = conflict_set
                .iter()
                .filter_map(|&lid| location_map.iter().find(|(_, loc)| loc.location_id == lid).map(|(code, _)| code.clone()))
                .collect();
            result.failed_count += conflict_set.len();
            result.errors.push(format!(
                "库位已被其它产品占用: {}",
                conflict_codes.join(", ")
            ));
        }

        // 过滤掉冲突的条目
        pending_items.retain(|item| {
            item.location_id.is_none_or(|lid| !conflict_set.contains(&lid))
        });

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
        INSERT INTO product_price (product_id, price, operator_id, remark)
        VALUES ($1, $2, NULL, 'Excel 批量导入更新')
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
