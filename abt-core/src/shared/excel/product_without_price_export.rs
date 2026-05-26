//! 无价格产品导出实现
//!
//! 导出缺少价格的产品，使用导入格式，方便填写价格后重新导入。

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::wms::stock_ledger::repo::StockLedgerRepo;

/// 无价格产品导出列定义
const PRODUCT_WITHOUT_PRICE_HEADERS: [&str; 5] = [
    "产品ID", "产品名称", "产品编码", "规格", "单位",
];

/// 无价格产品 Excel 导出器
pub struct ProductWithoutPriceExporter {
    pool: PgPool,
}

impl ProductWithoutPriceExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出没有价格记录的产品到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let rows = StockLedgerRepo::find_products_without_price(&mut conn).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &PRODUCT_WITHOUT_PRICE_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, row.product_id as f64)?;
            worksheet.write_string(row_num, 1, &row.pdt_name)?;
            worksheet.write_string(row_num, 2, &row.product_code)?;
            worksheet.write_string(row_num, 3, row.specification.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 4, row.unit.as_deref().unwrap_or(""))?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
