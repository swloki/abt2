use std::io::{BufReader, Cursor};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use anyhow::{Context, Result};
use calamine::{Data, Range, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use rust_xlsxwriter::Worksheet;
use serde::Deserialize;

use super::types::{ImportProgress, ImportSource};

pub struct ProgressTracker {
    current: AtomicUsize,
    total: AtomicUsize,
}

impl ProgressTracker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            current: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
        })
    }

    pub fn set_total(&self, n: usize) {
        self.total.store(n, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn tick(&self) {
        self.current.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ImportProgress {
        ImportProgress {
            current: self.current.load(std::sync::atomic::Ordering::Relaxed),
            total: self.total.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

/// 从 ImportSource 获取第一个工作表的 Range
pub fn import_range_from_source(source: ImportSource) -> Result<Range<Data>> {
    match source {
        ImportSource::Path(path) => {
            let mut excel: Xlsx<_> =
                open_workbook(&path).context("无法打开 Excel 文件")?;
            excel
                .worksheet_range_at(0)
                .ok_or_else(|| anyhow::anyhow!("找不到第一个工作表"))?
                .context("无法读取工作表")
        }
        ImportSource::Bytes(bytes) => {
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
pub fn write_headers(worksheet: &mut Worksheet, headers: &[&str]) -> Result<()> {
    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string(0, col as u16, *header)?;
    }
    Ok(())
}

/// 将 `Option<String>` 反序列化为 `Option<Decimal>`，空字符串视为 None
pub fn deserialize_optional_decimal<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Decimal::from_str_exact(&s).map(Some).map_err(serde::de::Error::custom),
    }
}
