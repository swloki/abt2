//! 分类 Excel 导出实现

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;

const CATEGORY_EXPORT_HEADERS: [&str; 2] = ["分类ID", "分类名称"];

/// 分类导出行结构
#[derive(Debug, sqlx::FromRow)]
struct CategoryExportRow {
    category_id: i64,
    category_name: String,
}

/// 分类 Excel 导出器
pub struct CategoryExporter {
    pool: PgPool,
}

impl CategoryExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出所有分类到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let rows = sqlx::query_as::<_, CategoryExportRow>(
            r#"
            SELECT category_id, category_name
            FROM categories
            WHERE deleted_at IS NULL
            ORDER BY category_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &CATEGORY_EXPORT_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, row.category_id as f64)?;
            worksheet.write_string(row_num, 1, &row.category_name)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
