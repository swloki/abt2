use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait PrintTemplateService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePrintTemplateReq,
    ) -> Result<i64>;

    async fn get(&self, db: PgExecutor<'_>, id: i64) -> Result<PrintTemplate>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdatePrintTemplateReq,
    ) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn list(
        &self,
        db: PgExecutor<'_>,
        filter: PrintTemplateQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PrintTemplate>>;

    /// 列出指定 document_type 的全部模板（不分页，打印按钮下拉选择用）。
    /// 按 is_default DESC、created_at DESC 排序——默认模板置顶。
    async fn list_by_document_type(
        &self,
        db: PgExecutor<'_>,
        document_type: &str,
    ) -> Result<Vec<PrintTemplate>>;

    async fn set_default(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 用 minijinja (Jinja2 语法) 渲染模板。vars 为 serde_json::Value，
    /// Object 顶层 key 即模板变量名（支持中文，需 minijinja `unicode` feature）。
    async fn render(
        &self,
        db: PgExecutor<'_>,
        template_id: i64,
        vars: RenderVars,
    ) -> Result<String>;

    /// 找到指定 document_type 的默认模板并用 minijinja 渲染。
    async fn render_default(
        &self,
        db: PgExecutor<'_>,
        document_type: &str,
        vars: RenderVars,
    ) -> Result<String>;
}
