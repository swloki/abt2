//! 无价格产品导出实现
//!
//! 导出缺少价格的产品，使用导入格式，方便填写价格后重新导入。

use anyhow::Result;
use async_trait::async_trait;
use rust_xlsxwriter::Workbook;
use sqlx::{FromRow, PgPool};

use super::product_inventory_import::PRODUCT_IMPORT_HEADERS;
use crate::implt::excel::write_headers;
use crate::service::{ExcelExportService, ExportRequest};

#[derive(Debug, FromRow)]
struct ProductWithoutPriceRow {
    pdt_name: String,
    product_code: String,
    old_code: String,
}

pub struct ProductWithoutPriceExporter {
    pool: PgPool,
}

impl ProductWithoutPriceExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExcelExportService for ProductWithoutPriceExporter {
    type Params = ();

    async fn export(&self, _req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
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
        .fetch_all(&self.pool)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &PRODUCT_IMPORT_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &row.product_code)?;
            if !row.old_code.is_empty() {
                worksheet.write_string(row_num, 1, &row.old_code)?;
            }
            worksheet.write_string(row_num, 2, &row.pdt_name)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
