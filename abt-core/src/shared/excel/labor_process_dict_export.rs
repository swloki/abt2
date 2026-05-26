//! 工序字典 Excel 导出实现

use anyhow::Result;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::master_data::labor_process_dict::repo::LaborProcessDictRepo;

const DICT_EXPORT_HEADERS: [&str; 5] = ["工序ID", "工序编码", "工序名称", "描述", "排序"];

/// 工序字典 Excel 导出器
pub struct LaborProcessDictExporter {
    pool: PgPool,
}

impl LaborProcessDictExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出所有工序字典到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let items = LaborProcessDictRepo {}.list_all(&mut conn).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &DICT_EXPORT_HEADERS)?;

        for (row_idx, d) in items.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, d.id as f64)?;
            worksheet.write_string(row_num, 1, &d.code)?;
            worksheet.write_string(row_num, 2, &d.name)?;
            worksheet.write_string(row_num, 3, d.description.as_deref().unwrap_or(""))?;
            worksheet.write_number(row_num, 4, d.sort_order as f64)?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
