use serde::{Deserialize, Serialize};

/// 导入来源
#[derive(Debug, Clone)]
pub enum ImportSource {
    Path(std::path::PathBuf),
    Bytes(Vec<u8>),
}

/// 导入进度
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportProgress {
    pub current: usize,
    pub total: usize,
}

/// 行级错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowError {
    pub row_index: usize,
    pub column_name: String,
    pub reason: String,
    pub raw_value: Option<String>,
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportResult {
    pub success_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub row_errors: Vec<RowError>,
}

/// 导出请求
#[derive(Debug, Clone)]
pub struct ExportQuery {
    pub format: String,
    pub page: u32,
    pub page_size: u32,
}
