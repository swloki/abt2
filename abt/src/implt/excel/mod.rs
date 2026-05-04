//! Excel 服务实现模块
//!
//! 每个导入/导出操作对应一个独立文件。

mod bom_export;
mod boms_no_labor_cost_export;
mod labor_process_dict_export;
mod labor_process_export;
mod labor_process_import;
mod product_all_export;
mod product_inventory_import;
mod product_without_price_export;
mod progress;
mod warehouse_location_export;
mod warehouse_location_import;

use anyhow::{Context, Result};
use calamine::{Range, Data, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use rust_xlsxwriter::Worksheet;
use serde::Deserialize;
use serde::Deserializer;

use crate::service::ImportSource;

pub use bom_export::BomExporter;
pub use boms_no_labor_cost_export::BomsWithoutLaborCostExporter;
pub use labor_process_dict_export::LaborProcessDictExporter;
pub use labor_process_export::LaborProcessExporter;
pub use labor_process_import::{normalize_process_name, LaborProcessImporter};
pub use product_all_export::ProductAllExporter;
pub use product_inventory_import::ProductInventoryImporter;
pub use product_without_price_export::ProductWithoutPriceExporter;
pub use progress::ProgressTracker;
pub use warehouse_location_export::export_warehouse_locations_to_bytes;
pub use warehouse_location_import::import_warehouse_locations;

// ---- 共享辅助函数 ----

/// 将 `Option<String>` 反序列化为 `Option<Decimal>`，空字符串视为 None
pub(crate) fn deserialize_optional_decimal<'de, D>(deserializer: D) -> std::result::Result<Option<Decimal>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Decimal::from_str_exact(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// 从 `ImportSource` 获取第一个工作表的 Range，消耗 source 避免克隆
pub(crate) fn import_range_from_source(source: ImportSource) -> Result<Range<Data>> {
    match source {
        ImportSource::Path(path) => {
            let mut excel: Xlsx<_> = open_workbook(&path).context("无法打开 Excel 文件")?;
            excel
                .worksheet_range_at(0)
                .ok_or_else(|| anyhow::anyhow!("找不到第一个工作表"))?
                .context("无法读取工作表")
        }
        ImportSource::Bytes(bytes) => {
            use std::io::{BufReader, Cursor};
            let cursor = Cursor::new(bytes);
            let mut excel: Xlsx<_> = Xlsx::new(BufReader::new(cursor))
                .map_err(|e| anyhow::anyhow!("无法读取 Excel 数据: {}", e))?;
            excel
                .worksheet_range_at(0)
                .ok_or_else(|| anyhow::anyhow!("找不到第一个工作表"))?
                .context("无法读取工作表")
        }
    }
}

/// 将表头写入工作表的第一行
pub(crate) fn write_headers(worksheet: &mut Worksheet, headers: &[&str]) -> Result<()> {
    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string(0, col as u16, *header)?;
    }
    Ok(())
}
