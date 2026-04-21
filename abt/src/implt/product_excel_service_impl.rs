//! 产品 Excel 服务实现
//!
//! 实现产品 Excel 导入导出的业务逻辑。
//! 使用批量操作优化导入性能。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use calamine::{RangeDeserializerBuilder, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;

use sqlx::FromRow;

use crate::repositories::{InventoryRepo, LocationRepo, ProductRepo};
use crate::service::{ExcelProgress, ImportResult, ProductExcelService};

/// 反序列化 Decimal，空字符串转为 None
fn deserialize_empty_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Decimal::from_str_exact(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Excel 行数据结构
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
    #[serde(rename = "库存数量", deserialize_with = "deserialize_empty_decimal")]
    quantity: Option<Decimal>,
    #[serde(rename = "价格", deserialize_with = "deserialize_empty_decimal")]
    price: Option<Decimal>,
    #[serde(rename = "安全库存", deserialize_with = "deserialize_empty_decimal")]
    safety_stock: Option<Decimal>,
}

/// 待处理的数据项
struct PendingItem {
    product_id: i64,
    location_id: Option<i64>,
    quantity: Option<Decimal>,
    price: Option<Decimal>,
    safety_stock: Option<Decimal>,
    new_name: Option<String>,
}

/// 产品 Excel 服务实现
pub struct ProductExcelServiceImpl {
    total_count: AtomicUsize,
    current_count: AtomicUsize,
}

impl ProductExcelServiceImpl {
    pub fn new() -> Self {
        Self {
            total_count: AtomicUsize::new(0),
            current_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ProductExcelService for ProductExcelServiceImpl {
    /// 从 Excel 导入库存数据（批量优化版本）
    async fn import_quantity_from_excel(
        &self,
        pool: &PgPool,
        path: &Path,
        _operator_id: Option<i64>,
    ) -> Result<ImportResult> {
        let mut result = ImportResult::default();

        // 1. 打开 Excel 文件并解析所有行
        let mut excel: Xlsx<_> = open_workbook(path).context("无法打开 Excel 文件")?;
        let range = excel
            .worksheet_range_at(0)
            .ok_or_else(|| anyhow!("找不到第一个工作表"))?
            .context("无法读取工作表")?;

        let headers = [
            "新编码", "旧编码", "物料名称", "仓库名称", "库位名称", "库存数量", "价格", "安全库存",
        ];
        let iter_results = RangeDeserializerBuilder::with_headers(&headers).from_range(&range)?;

        let total = range.rows().count().saturating_sub(1);
        self.total_count.store(total, Ordering::SeqCst);
        self.current_count.store(0, Ordering::SeqCst);

        // 2. 解析所有行
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

        // 3. 收集所有产品编码
        let mut all_codes: Vec<String> = Vec::new();
        for row in &rows {
            if !row.new_code.is_empty() {
                all_codes.push(row.new_code.clone());
            }
            if let Some(ref old_code) = row.old_code
                && !old_code.is_empty() {
                    all_codes.push(old_code.clone());
                }
        }
        all_codes.sort();
        all_codes.dedup();

        // 4. 批量查询产品
        let products = ProductRepo::find_by_codes(pool, &all_codes).await?;
        let product_map: HashMap<String, (i64, String)> = products
            .iter()
            .filter(|p| !p.meta.product_code.is_empty())
            .map(|p| (p.meta.product_code.clone(), (p.product_id, p.pdt_name.clone())))
            .collect();

        // 5. 批量查询库位
        let location_map = LocationRepo::list_all_with_warehouse(pool).await?;

        // 6. 构建待处理数据
        let mut pending_items: Vec<PendingItem> = Vec::with_capacity(rows.len());
        for row in &rows {
            // 查找产品
            let (product_id, db_name) = if let Some(&(id, ref name)) = product_map.get(&row.new_code) {
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

            // 查找库位
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

            // 仅当 Excel 中的名称非空且与数据库不同时才更新产品名称
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

        // 7. 批量执行数据库操作
        let mut tx = pool.begin().await?;

        for item in &pending_items {
            // 每处理一条记录都更新进度，确保进度始终达到 100%
            self.current_count.fetch_add(1, Ordering::SeqCst);

            // 更新产品名称
            if let Some(ref name) = item.new_name
                && let Err(e) = ProductRepo::update_name(&mut *tx, item.product_id, name).await {
                    result.failed_count += 1;
                    result.errors.push(format!("更新产品名称失败 product_id={}: {}", item.product_id, e));
                    continue;
                }

            // 更新价格
            if let Some(price) = item.price
                && let Err(e) = update_price_batch(&mut tx, item.product_id, price).await {
                    result.failed_count += 1;
                    result.errors.push(format!("更新价格失败 product_id={}: {}", item.product_id, e));
                    continue;
                }

            // 更新库存和安全库存
            if let Some(location_id) = item.location_id {
                if let Some(quantity) = item.quantity
                    && let Err(e) = upsert_inventory_quantity(&mut tx, item.product_id, location_id, quantity).await {
                        result.failed_count += 1;
                        result.errors.push(format!("更新库存失败: {}", e));
                        continue;
                    }

                if let Some(safety_stock) = item.safety_stock
                    && let Err(e) = upsert_inventory_safety_stock(&mut tx, item.product_id, location_id, safety_stock).await {
                        result.failed_count += 1;
                        result.errors.push(format!("更新安全库存失败: {}", e));
                        continue;
                    }
            }

            result.success_count += 1;
        }

        tx.commit().await?;

        Ok(result)
    }

    /// 导出产品到 Excel（详细格式，每行一个库位）
    async fn export_products_to_excel(&self, pool: &PgPool, path: &Path) -> Result<()> {
        let rows = InventoryRepo::list_for_export(pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        let headers = [
            "产品ID", "产品名称", "产品编码", "规格", "单位", "仓库名称", "库位编码",
            "库存数量", "安全库存", "价格",
        ];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = row_idx + 1;
            worksheet.write_number(row_num as u32, 0, row.product_id as f64)?;
            worksheet.write_string(row_num as u32, 1, &row.pdt_name)?;
            worksheet.write_string(row_num as u32, 2, &row.product_code)?;
            worksheet.write_string(row_num as u32, 3, &row.specification)?;
            worksheet.write_string(row_num as u32, 4, &row.unit)?;
            worksheet.write_string(row_num as u32, 5, &row.warehouse_name)?;
            worksheet.write_string(row_num as u32, 6, &row.location_code)?;
            worksheet.write_number(row_num as u32, 7, row.quantity.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num as u32, 8, row.safety_stock.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num as u32, 9, row.price.to_f64().unwrap_or(0.0))?;
        }

        workbook.save(path)?;
        Ok(())
    }

    /// 导出产品到 Excel（返回字节数据，用于流式下载）
    async fn export_products_to_bytes(&self, pool: &PgPool) -> Result<Vec<u8>> {
        let rows = InventoryRepo::list_for_export(pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        let headers = [
            "产品ID", "产品名称", "产品编码", "规格", "单位", "仓库名称", "库位编码",
            "库存数量", "安全库存", "价格",
        ];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = row_idx + 1;
            worksheet.write_number(row_num as u32, 0, row.product_id as f64)?;
            worksheet.write_string(row_num as u32, 1, &row.pdt_name)?;
            worksheet.write_string(row_num as u32, 2, &row.product_code)?;
            worksheet.write_string(row_num as u32, 3, &row.specification)?;
            worksheet.write_string(row_num as u32, 4, &row.unit)?;
            worksheet.write_string(row_num as u32, 5, &row.warehouse_name)?;
            worksheet.write_string(row_num as u32, 6, &row.location_code)?;
            worksheet.write_number(row_num as u32, 7, row.quantity.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num as u32, 8, row.safety_stock.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num as u32, 9, row.price.to_f64().unwrap_or(0.0))?;
        }

        // 保存到内存缓冲区
        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }

    /// 获取处理进度
    fn get_progress(&self) -> ExcelProgress {
        ExcelProgress {
            current: self.current_count.load(Ordering::SeqCst),
            total: self.total_count.load(Ordering::SeqCst),
        }
    }

    /// 导出没有价格的产品（导入格式）
    async fn export_products_without_price_to_bytes(&self, pool: &PgPool) -> Result<Vec<u8>> {
        let rows = sqlx::query_as::<_, ProductWithoutPriceRow>(
            r#"
            SELECT
                p.pdt_name,
                COALESCE(p.meta->>'product_code', '') as product_code,
                COALESCE(p.meta->>'old_code', '') as old_code
            FROM products p
            WHERE (p.meta->>'price') IS NULL
               OR (p.meta->>'price') = ''
               OR (p.meta->>'price')::decimal = 0
            ORDER BY p.pdt_name
            "#,
        )
        .fetch_all(pool)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // 导入格式表头
        let headers = ["新编码", "旧编码", "物料名称", "仓库名称", "库位名称", "库存数量", "价格", "安全库存"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &row.product_code)?;
            if !row.old_code.is_empty() {
                worksheet.write_string(row_num, 1, &row.old_code)?;
            }
            worksheet.write_string(row_num, 2, &row.pdt_name)?;
            // 仓库名称、库位名称、库存数量、价格、安全库存 留空
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }

}

// ============================================================================
// 导出查询行结构
// ============================================================================

/// 没有价格的产品行
#[derive(Debug, FromRow)]
struct ProductWithoutPriceRow {
    pdt_name: String,
    product_code: String,
    old_code: String,
}

// ============================================================================
// 批量操作辅助函数
// ============================================================================

/// 批量更新价格
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

/// 批量更新库存数量（UPSERT）
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

/// 批量更新安全库存（UPSERT）
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
