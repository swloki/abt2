//! 缺少人工成本的 BOM 导出实现

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::master_data::bom_labor_process::repo::BomLaborProcessRepo;

/// 缺少人工成本的 BOM 导出列定义（schema-as-code）
const BOMS_NO_COST_COLUMNS: [&str; 3] = ["BOM ID", "BOM 名称", "产品编码"];

/// 缺少劳务成本的 BOM Excel 导出器
pub struct BomsNoLaborCostExporter {
    pool: PgPool,
}

impl BomsNoLaborCostExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出没有劳务成本的 BOM 到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let boms = BomLaborProcessRepo::find_boms_without_labor_cost(&mut conn).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &BOMS_NO_COST_COLUMNS)?;

        for (row_idx, b) in boms.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, b.bom_id as f64)?;
            worksheet.write_string(row_num, 1, &b.bom_name)?;
            worksheet.write_string(row_num, 2, &b.product_code)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
