//! 仓库库位 Excel 导出实现
//!
//! 导出所有未删除的仓库及其库位到 Excel 文件。

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

/// 导出查询行结构
#[derive(Debug, sqlx::FromRow)]
struct WarehouseLocationExportRow {
    warehouse_code: String,
    warehouse_name: String,
    location_code: Option<String>,
    location_name: Option<String>,
    capacity: Option<i32>,
}

/// 导出所有仓库和库位到 Excel 字节数据
pub async fn export_warehouse_locations_to_bytes(pool: &PgPool) -> Result<Vec<u8>> {
    let rows = sqlx::query_as::<_, WarehouseLocationExportRow>(
        r#"
        SELECT
            w.warehouse_code,
            w.warehouse_name,
            l.location_code,
            l.location_name,
            l.capacity
        FROM warehouse w
        JOIN location l ON w.warehouse_id = l.warehouse_id AND l.deleted_at IS NULL
        WHERE w.deleted_at IS NULL
        ORDER BY w.warehouse_code, l.location_code
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let headers = ["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"];
    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string(0, col as u16, *header)?;
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let row_num = (row_idx + 1) as u32;
        worksheet.write_string(row_num, 0, &row.warehouse_code)?;
        worksheet.write_string(row_num, 1, &row.warehouse_name)?;
        if let Some(ref loc_code) = row.location_code {
            worksheet.write_string(row_num, 2, loc_code)?;
        }
        if let Some(ref loc_name) = row.location_name {
            worksheet.write_string(row_num, 3, loc_name)?;
        }
        if let Some(capacity) = row.capacity {
            worksheet.write_number(row_num, 4, capacity as f64)?;
        }
    }

    let bytes = workbook.save_to_buffer()?;
    Ok(bytes)
}
