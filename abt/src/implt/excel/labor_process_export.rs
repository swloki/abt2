//! 工序 Excel 导出实现

use anyhow::{Context, Result};
use async_trait::async_trait;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::implt::excel::write_headers;
use crate::models::LABOR_PROCESS_EXCEL_COLUMNS;
use crate::repositories::LaborProcessRepo;
use crate::service::{ExcelExportService, ExportRequest};

pub struct LaborProcessExporter {
    pool: PgPool,
}

impl LaborProcessExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExcelExportService for LaborProcessExporter {
    type Params = String;

    async fn export(&self, req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
        let product_code = req.params;
        let processes = LaborProcessRepo::list_all_by_product_code(&self.pool, &product_code).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &LABOR_PROCESS_EXCEL_COLUMNS)?;

        for (row_idx, p) in processes.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &p.product_code)?;
            worksheet.write_string(row_num, 1, p.process_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 2, &p.name)?;
            worksheet.write_number(row_num, 3, p.unit_price.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 4, p.quantity.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 5, p.sort_order as f64)?;
            worksheet.write_string(row_num, 6, p.remark.as_deref().unwrap_or(""))?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
