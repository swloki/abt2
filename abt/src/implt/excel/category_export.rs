//! 分类 (terms) Excel 导出实现

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::implt::excel::write_headers;

const CATEGORY_EXPORT_HEADERS: [&str; 2] = ["分类ID", "分类名称"];

#[derive(Debug, sqlx::FromRow)]
struct CategoryExportRow {
    term_id: i64,
    term_name: String,
}

pub async fn export_categories_to_bytes(pool: &PgPool) -> Result<Vec<u8>> {
    let rows = sqlx::query_as::<_, CategoryExportRow>(
        r#"
        SELECT term_id, term_name
        FROM terms
        WHERE taxonomy = 'category'
        ORDER BY term_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    write_headers(worksheet, &CATEGORY_EXPORT_HEADERS)?;

    for (row_idx, row) in rows.iter().enumerate() {
        let row_num = (row_idx + 1) as u32;
        worksheet.write_number(row_num, 0, row.term_id as f64)?;
        worksheet.write_string(row_num, 1, &row.term_name)?;
    }

    Ok(workbook.save_to_buffer()?)
}
