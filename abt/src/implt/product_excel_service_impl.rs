//! 产品 Excel 服务实现
//!
//! 实现产品 Excel 导入导出的业务逻辑。
//! 使用新的 InventoryService 和 ProductPriceService。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use calamine::{RangeDeserializerBuilder, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use serde::Deserialize;
use sqlx::PgPool;

use crate::models::{Location, OperationType, SetSafetyStockRequest, StockChangeRequest};
use crate::repositories::{Executor, InventoryRepo, LocationRepo, ProductRepo};
use crate::service::{
    ExcelProgress, ImportResult, InventoryService, ProductExcelService, ProductPriceService,
};

/// Excel 行数据结构
#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "新编码")]
    new_code: String,
    #[serde(rename = "旧编码")]
    old_code: Option<String>,
    #[serde(rename = "物料名称")]
    _product_name: String,
    #[serde(rename = "仓库名称")]
    warehouse_name: String,
    #[serde(rename = "库位名称", alias = "库位")]
    storage: Option<String>,
    #[serde(rename = "库存数量")]
    quantity: Decimal,
    #[serde(rename = "价格")]
    price: Option<Decimal>,
    #[serde(rename = "安全库存")]
    safety_stock: Option<Decimal>,
}

/// 产品 Excel 服务实现
pub struct ProductExcelServiceImpl {
    total_count: AtomicUsize,
    current_count: AtomicUsize,
    price_service: Arc<dyn ProductPriceService>,
    inventory_service: Arc<dyn InventoryService>,
}

impl ProductExcelServiceImpl {
    pub fn new(
        price_service: Arc<dyn ProductPriceService>,
        inventory_service: Arc<dyn InventoryService>,
    ) -> Self {
        Self {
            total_count: AtomicUsize::new(0),
            current_count: AtomicUsize::new(0),
            price_service,
            inventory_service,
        }
    }

    /// 处理单行 Excel 数据
    async fn process_excel_row(
        &self,
        pool: &PgPool,
        row: ExcelRow,
        operator_id: Option<i64>,
    ) -> Result<()> {
        // 1. 查找产品（先按新编码，再按旧编码）
        let mut product = ProductRepo::find_by_code(pool, &row.new_code).await?;

        // 如果没找到且有旧编码，再按旧编码查找
        if product.is_none()
            && let Some(old_code) = &row.old_code
        {
            product = ProductRepo::find_by_code(pool, old_code).await?;
        }

        let product = product.ok_or_else(|| {
            anyhow!(
                "产品未找到: 新编码={}, 旧编码={}",
                row.new_code,
                row.old_code.as_deref().unwrap_or_default()
            )
        })?;

        // 2. 查找库位（使用 Repository）
        let location = find_location(pool, &row.warehouse_name, &row.storage).await?;

        // 3. 开启事务
        let mut tx = pool.begin().await?;

        // 4. 更新价格（如果有）
        if let Some(price) = row.price {
            self.price_service
                .update_price(
                    product.product_id,
                    price,
                    operator_id,
                    Some("Excel 导入更新"),
                    Executor::from(&mut *tx),
                )
                .await?;
        }

        // 5. 盘点设置库存（直接设置为 Excel 中的数量）
        let req = StockChangeRequest {
            product_id: product.product_id,
            location_id: location.location_id,
            quantity: row.quantity,
            operation_type: OperationType::Adjust,
            operator: operator_id.map(|id| id.to_string()),
            remark: Some("Excel 盘点导入".to_string()),
            ..Default::default()
        };
        self.inventory_service
            .set_quantity(req, Executor::from(&mut *tx))
            .await?;

        // 6. 设置安全库存（如果有）
        if let Some(safety_stock) = row.safety_stock {
            let req = SetSafetyStockRequest {
                product_id: product.product_id,
                location_id: location.location_id,
                safety_stock,
            };
            self.inventory_service
                .set_safety_stock(req, Executor::from(&mut *tx))
                .await?;
        }

        // 7. 提交事务
        tx.commit().await?;

        Ok(())
    }
}

#[async_trait]
impl ProductExcelService for ProductExcelServiceImpl {
    /// 从 Excel 导入库存数据
    async fn import_quantity_from_excel(
        &self,
        pool: &PgPool,
        path: &Path,
        operator_id: Option<i64>,
    ) -> Result<ImportResult> {
        let mut result = ImportResult::default();

        // 打开 Excel 文件
        let mut excel: Xlsx<_> = open_workbook(path).context("无法打开 Excel 文件")?;

        let range = excel
            .worksheet_range_at(0)
            .ok_or_else(|| anyhow!("找不到第一个工作表"))?
            .context("无法读取工作表")?;

        // 定义表头（支持价格和安全库存列）
        let headers = [
            "新编码",
            "旧编码",
            "物料名称",
            "仓库名称",
            "库位名称",
            "库存数量",
            "价格",
            "安全库存",
        ];

        let iter_results = RangeDeserializerBuilder::with_headers(&headers).from_range(&range)?;

        // 统计总数（减去表头）
        let total = range.rows().count().saturating_sub(1);
        self.total_count.store(total, Ordering::SeqCst);
        self.current_count.store(0, Ordering::SeqCst);

        // 处理每一行
        for row_result in iter_results {
            let row: ExcelRow = match row_result {
                Ok(r) => r,
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(format!("解析 Excel 行失败: {}", e));
                    self.current_count.fetch_add(1, Ordering::SeqCst);
                    continue;
                }
            };

            match self.process_excel_row(pool, row, operator_id).await {
                Ok(_) => {
                    result.success_count += 1;
                }
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(e.to_string());
                }
            }

            // 更新进度
            self.current_count.fetch_add(1, Ordering::SeqCst);
        }

        Ok(result)
    }

    /// 导出产品到 Excel（详细格式，每行一个库位）
    async fn export_products_to_excel(&self, pool: &PgPool, path: &Path) -> Result<()> {
        // 使用 Repository 查询库存数据
        let rows = InventoryRepo::list_for_export(pool).await?;

        // 创建 Excel 工作簿
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // 写入表头
        let headers = [
            "产品ID",
            "产品名称",
            "产品编码",
            "规格",
            "单位",
            "仓库名称",
            "库位编码",
            "库存数量",
            "安全库存",
            "价格",
        ];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        // 写入数据
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

        // 保存文件
        workbook.save(path)?;

        Ok(())
    }

    /// 获取处理进度
    fn get_progress(&self) -> ExcelProgress {
        ExcelProgress {
            current: self.current_count.load(Ordering::SeqCst),
            total: self.total_count.load(Ordering::SeqCst),
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 查找库位（使用 Repository）
async fn find_location(
    pool: &PgPool,
    warehouse_name: &str,
    location_name: &Option<String>,
) -> Result<Location> {
    match location_name {
        Some(loc_name) if !loc_name.is_empty() => {
            // 有库位名称时，严格匹配仓库+库位
            LocationRepo::find_by_warehouse_name_and_code(pool, warehouse_name, loc_name)
                .await?
                .ok_or_else(|| anyhow!("库位未找到: 仓库={}, 库位={}", warehouse_name, loc_name))
        }
        _ => {
            // 无库位名称时，使用仓库的默认库位（第一个库位）
            LocationRepo::find_default_by_warehouse_name(pool, warehouse_name)
                .await?
                .ok_or_else(|| anyhow!("仓库未找到或无默认库位: {}", warehouse_name))
        }
    }
}
