//! 产品全量导出实现

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::implt::excel::write_headers;
use crate::repositories::InventoryRepo;
use crate::service::{ExcelExportService, ExportRequest};

/// 产品全量导出列定义（schema-as-code）
pub const PRODUCT_EXPORT_HEADERS: [&str; 10] = [
    "产品ID", "产品名称", "产品编码", "规格", "单位", "仓库名称", "库位编码",
    "库存数量", "安全库存", "价格",
];
const _: () = assert!(PRODUCT_EXPORT_HEADERS.len() == 10);

pub struct ProductAllExporter {
    pool: PgPool,
}

impl ProductAllExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExcelExportService for ProductAllExporter {
    type Params = ();

    async fn export(&self, _req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
        let rows = InventoryRepo::list_for_export(&self.pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &PRODUCT_EXPORT_HEADERS)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, row.product_id as f64)?;
            worksheet.write_string(row_num, 1, &row.pdt_name)?;
            worksheet.write_string(row_num, 2, &row.product_code)?;
            worksheet.write_string(row_num, 3, &row.specification)?;
            worksheet.write_string(row_num, 4, &row.unit)?;
            worksheet.write_string(row_num, 5, &row.warehouse_name)?;
            worksheet.write_string(row_num, 6, &row.location_code)?;
            worksheet.write_number(row_num, 7, row.quantity.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num, 8, row.safety_stock.to_f64().unwrap_or(0.0))?;
            worksheet.write_number(row_num, 9, row.price.to_f64().unwrap_or(0.0))?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
