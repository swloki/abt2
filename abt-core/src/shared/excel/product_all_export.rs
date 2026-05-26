//! 产品全量导出实现

use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::wms::stock_ledger::repo::StockLedgerRepo;

/// 产品全量导出列定义（schema-as-code）
const PRODUCT_EXPORT_HEADERS: [&str; 13] = [
    "产品ID", "产品名称", "产品编码", "规格", "单位", "仓库名称", "区域编码", "库位编码",
    "库存数量", "安全库存", "价格", "分类ID", "分类名称",
];

/// 产品全量 Excel 导出器
pub struct ProductAllExporter {
    pool: PgPool,
}

impl ProductAllExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出所有产品库存到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let rows = StockLedgerRepo::list_for_export(&mut conn).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &PRODUCT_EXPORT_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, row.product_id as f64)?;
            worksheet.write_string(row_num, 1, &row.pdt_name)?;
            worksheet.write_string(row_num, 2, &row.product_code)?;
            worksheet.write_string(row_num, 3, row.specification.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 4, row.unit.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 5, row.warehouse_name.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 6, row.zone_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 7, row.bin_code.as_deref().unwrap_or(""))?;
            worksheet.write_number(row_num, 8, row.quantity.unwrap_or_default().to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num, 9, row.safety_stock.unwrap_or_default().to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num, 10, row.price.unwrap_or_default().to_f64().unwrap_or(0.0))?;
            if let Some(ref ids) = row.category_ids {
                worksheet.write_string(row_num, 11, ids)?;
            }
            if let Some(ref names) = row.category_names {
                worksheet.write_string(row_num, 12, names)?;
            }
        }

        Ok(workbook.save_to_buffer()?)
    }
}
