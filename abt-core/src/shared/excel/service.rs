use async_trait::async_trait;

use super::types::*;
use crate::shared::types::{DomainError, PaginatedResult, ServiceContext};

/// Excel 导入服务 — 每个导入场景独立实现此 trait
#[async_trait]
pub trait ExcelImportService: Send + Sync {
    async fn start_import(&self, ctx: ServiceContext<'_>, source: ImportSource) -> Result<i64, DomainError>;
    async fn get_import_progress(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ImportProgress, DomainError>;
    async fn list_import_history(&self, ctx: ServiceContext<'_>, page: u32, page_size: u32) -> Result<PaginatedResult<ImportResult>, DomainError>;
    async fn cancel_import(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}

/// Excel 导出服务 — 每个导出场景独立实现此 trait
#[async_trait]
pub trait ExcelExportService: Send + Sync {
    async fn start_export(&self, ctx: ServiceContext<'_>, query: ExportQuery) -> Result<i64, DomainError>;
    async fn get_export_status(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ImportProgress, DomainError>;
    async fn download(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Vec<u8>, DomainError>;
    async fn list_export_history(&self, ctx: ServiceContext<'_>, page: u32, page_size: u32) -> Result<PaginatedResult<ImportResult>, DomainError>;
}
