//! 工序字典 Excel 导出实现

use anyhow::Result;
use async_trait::async_trait;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::implt::excel::write_headers;
use crate::repositories::LaborProcessDictRepo;
use crate::service::{ExcelExportService, ExportRequest};

/// 工序字典导出列定义（schema-as-code）
pub const DICT_EXPORT_COLUMNS: [&str; 4] = ["工序编码", "工序名称", "说明", "排序"];
const _: () = assert!(DICT_EXPORT_COLUMNS.len() == 4);

pub struct LaborProcessDictExporter {
    pool: PgPool,
}

impl LaborProcessDictExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExcelExportService for LaborProcessDictExporter {
    type Params = ();

    async fn export(&self, _req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
        let items = LaborProcessDictRepo::list_all(&self.pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &DICT_EXPORT_COLUMNS)?;

        for (row_idx, d) in items.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &d.code)?;
            worksheet.write_string(row_num, 1, &d.name)?;
            worksheet.write_string(row_num, 2, d.description.as_deref().unwrap_or(""))?;
            worksheet.write_number(row_num, 3, d.sort_order as f64)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
