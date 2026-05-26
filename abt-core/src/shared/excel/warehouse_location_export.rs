//! 仓库库位 Excel 导出实现
//!
//! 导出所有未删除的仓库、库区、库位到 Excel 文件。

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::wms::warehouse::repo::{WarehouseExportRow, WarehouseRepo};

const WAREHOUSE_LOCATION_EXPORT_HEADERS: [&str; 9] = [
    "仓库ID",
    "仓库编码",
    "仓库名称",
    "区域ID",
    "区域编码",
    "区域名称",
    "库位ID",
    "库位编码",
    "库位名称",
];

/// 仓库库位 Excel 导出器
pub struct WarehouseLocationExporter {
    pool: PgPool,
}

impl WarehouseLocationExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出所有仓库、库区、库位到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let rows = WarehouseRepo::list_all_for_export(&mut conn).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &WAREHOUSE_LOCATION_EXPORT_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            write_warehouse_export_row(worksheet, row_num, row)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}

fn write_warehouse_export_row(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row_num: u32,
    row: &WarehouseExportRow,
) -> Result<()> {
    worksheet.write_number(row_num, 0, row.warehouse_id as f64)?;
    worksheet.write_string(row_num, 1, &row.warehouse_code)?;
    worksheet.write_string(row_num, 2, &row.warehouse_name)?;
    worksheet.write_number(row_num, 3, row.zone_id as f64)?;
    worksheet.write_string(row_num, 4, &row.zone_code)?;
    worksheet.write_string(row_num, 5, &row.zone_name)?;
    worksheet.write_number(row_num, 6, row.bin_id as f64)?;
    worksheet.write_string(row_num, 7, &row.bin_code)?;
    worksheet.write_string(row_num, 8, &row.bin_name)?;
    Ok(())
}
