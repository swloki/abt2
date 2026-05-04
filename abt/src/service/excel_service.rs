//! Excel 导入导出统一服务接口
//!
//! 定义 Excel 导入和导出 trait，以及共享类型。
//! 每个导入/导出操作对应一个独立实现。

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// Excel 导入结果
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    /// 成功导入数量
    pub success_count: usize,
    /// 失败数量
    pub failed_count: usize,
    /// 错误信息
    pub errors: Vec<String>,
    /// 行级错误详情
    pub row_errors: Vec<RowError>,
}

/// 行级错误详情
#[derive(Debug, Clone)]
pub struct RowError {
    /// 行号（1-based）
    pub row_index: usize,
    /// 列名
    pub column_name: String,
    /// 错误原因
    pub reason: String,
    /// 原始值
    pub raw_value: Option<String>,
}

/// Excel 处理进度
#[derive(Debug, Clone, Default)]
pub struct ExcelProgress {
    /// 当前处理数量
    pub current: usize,
    /// 总数量
    pub total: usize,
}

/// 导入数据来源
///
/// 支持从文件路径或内存中的字节数据导入，使导入逻辑可脱离文件系统进行纯内存测试。
#[derive(Debug, Clone)]
pub enum ImportSource {
    /// 从文件路径导入
    Path(PathBuf),
    /// 从内存字节数据导入（用于 upload-then-import 流程和测试）
    Bytes(Vec<u8>),
}

/// 导出请求参数
///
/// 泛型 wrapper，携带请求级参数（如 bom_id、product_code）。
/// 无参数导出使用 `ExportRequest<()>`。
#[derive(Debug, Clone)]
pub struct ExportRequest<T> {
    pub params: T,
}

/// Excel 导入服务
///
/// 每个导入操作对应一个实现此 trait 的结构体。
#[async_trait]
pub trait ExcelImportService: Send + Sync {
    /// 从指定数据源导入 Excel 数据
    async fn import(&self, source: ImportSource) -> Result<ImportResult>;
}

/// Excel 导出服务
///
/// 每个导出操作对应一个实现此 trait 的结构体。
/// `Params` 关联类型携带请求级参数；无参数导出使用 `()`。
#[async_trait]
pub trait ExcelExportService: Send + Sync {
    /// 导出请求参数类型
    type Params: Send + Sync;

    /// 导出 Excel 数据并返回字节
    async fn export(&self, req: ExportRequest<Self::Params>) -> Result<Vec<u8>>;
}
