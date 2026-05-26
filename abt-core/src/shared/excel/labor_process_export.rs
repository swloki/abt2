//! BOM 劳务工序 Excel 导出实现

use anyhow::{Context, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;

const LABOR_PROCESS_EXPORT_HEADERS: [&str; 7] = [
    "产品编码",
    "工序编码",
    "工序名称",
    "单价",
    "数量",
    "排序",
    "备注",
];

/// BOM 劳务工序导出行结构
#[derive(Debug, sqlx::FromRow)]
struct LaborProcessExportRow {
    product_code: String,
    process_code: Option<String>,
    name: String,
    unit_price: rust_decimal::Decimal,
    quantity: rust_decimal::Decimal,
    sort_order: i32,
    remark: Option<String>,
}

/// BOM 劳务工序 Excel 导出器
pub struct LaborProcessExporter {
    pool: PgPool,
    product_code: String,
}

impl LaborProcessExporter {
    pub fn new(pool: PgPool, product_code: String) -> Self {
        Self { pool, product_code }
    }

    /// 导出指定产品的劳动工序到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let rows = sqlx::query_as::<_, LaborProcessExportRow>(
            r#"
            SELECT product_code, process_code, name, unit_price, quantity, sort_order, remark
            FROM bom_labor_processes
            WHERE product_code = $1 AND deleted_at IS NULL
            ORDER BY sort_order
            "#,
        )
        .bind(&self.product_code)
        .fetch_all(&self.pool)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &LABOR_PROCESS_EXPORT_HEADERS)?;

        for (row_idx, p) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &p.product_code)?;
            worksheet.write_string(row_num, 1, p.process_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 2, &p.name)?;
            worksheet.write_number(
                row_num,
                3,
                p.unit_price
                    .to_f64()
                    .context("Decimal 转 f64 失败")?,
            )?;
            worksheet.write_number(
                row_num,
                4,
                p.quantity
                    .to_f64()
                    .context("Decimal 转 f64 失败")?,
            )?;
            worksheet.write_number(row_num, 5, p.sort_order as f64)?;
            worksheet.write_string(row_num, 6, p.remark.as_deref().unwrap_or(""))?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
