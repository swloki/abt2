use async_trait::async_trait;

use super::types::*;
use crate::shared::types::{PgExecutor,PaginatedResult, Result, ServiceContext};

/// Excel 导入服务 — 每个导入场景独立实现此 trait
#[async_trait]
pub trait ExcelImportService: Send + Sync {
    async fn start_import(&self, ctx: &ServiceContext, db: PgExecutor<'_>, source: ImportSource) -> Result<i64>;
    async fn get_import_progress(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ImportProgress>;
    async fn list_import_history(&self, ctx: &ServiceContext, db: PgExecutor<'_>, page: u32, page_size: u32) -> Result<PaginatedResult<ImportResult>>;
    async fn cancel_import(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}

/// Excel 导出服务 — 每个导出场景独立实现此 trait
#[async_trait]
pub trait ExcelExportService: Send + Sync {
    async fn start_export(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: ExportQuery) -> Result<i64>;
    async fn get_export_status(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ImportProgress>;
    async fn download(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Vec<u8>>;
    async fn list_export_history(&self, ctx: &ServiceContext, db: PgExecutor<'_>, page: u32, page_size: u32) -> Result<PaginatedResult<ImportResult>>;
}
