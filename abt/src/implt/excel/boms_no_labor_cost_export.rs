//! 缺少人工成本的 BOM 导出实现

use anyhow::Result;
use async_trait::async_trait;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::implt::excel::write_headers;
use crate::repositories::LaborProcessRepo;
use crate::service::{ExcelExportService, ExportRequest};

/// 缺少人工成本的 BOM 导出列定义（schema-as-code）
pub const BOMS_NO_COST_COLUMNS: [&str; 4] = ["BOM名称", "产品编码", "产品名称", "创建时间"];
const _: () = assert!(BOMS_NO_COST_COLUMNS.len() == 4);

pub struct BomsWithoutLaborCostExporter {
    pool: PgPool,
}

impl BomsWithoutLaborCostExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExcelExportService for BomsWithoutLaborCostExporter {
    type Params = ();

    async fn export(&self, _req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
        let boms = LaborProcessRepo::find_boms_without_labor_cost(&self.pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &BOMS_NO_COST_COLUMNS)?;

        for (row_idx, b) in boms.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &b.bom_name)?;
            worksheet.write_string(row_num, 1, b.product_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 2, b.product_name.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 3, b.created_at.format("%Y-%m-%d %H:%M").to_string())?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
