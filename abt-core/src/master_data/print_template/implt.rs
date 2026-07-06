use async_trait::async_trait;
use minijinja::{AutoEscape, Environment};

use super::model::*;
use super::repo::PrintTemplateRepo;
use crate::shared::types::{
    DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext,
};

pub struct PrintTemplateServiceImpl {
    repo: PrintTemplateRepo,
}

impl PrintTemplateServiceImpl {
    pub fn new() -> Self {
        Self {
            repo: PrintTemplateRepo,
        }
    }
}

#[async_trait]
impl super::service::PrintTemplateService for PrintTemplateServiceImpl {
    async fn create(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePrintTemplateReq,
    ) -> Result<i64> {
        if req.name.trim().is_empty() {
            return Err(DomainError::Validation("模板名称不能为空".into()));
        }
        if req.html_content.trim().is_empty() {
            return Err(DomainError::Validation("模板内容不能为空".into()));
        }

        // If setting as default, clear existing defaults for this document_type
        if req.is_default {
            self.repo.clear_default(db, &req.document_type).await?;
        }

        self.repo.create(db, &req).await
    }

    async fn get(&self, db: PgExecutor<'_>, id: i64) -> Result<PrintTemplate> {
        self.repo
            .get(db, id)
            .await?
            .ok_or(DomainError::NotFound("打印模板不存在".into()))
    }

    async fn update(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdatePrintTemplateReq,
    ) -> Result<()> {
        let existing = self.repo.get(db, id).await?.ok_or(DomainError::NotFound("打印模板不存在".into()))?;

        // If switching to default, clear existing defaults for this document_type
        if req.is_default == Some(true) {
            let dt = req.document_type.as_ref().unwrap_or(&existing.document_type);
            self.repo.clear_default(db, dt).await?;
        }

        self.repo.update(db, id, &req).await
    }

    async fn delete(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        // Verify existence
        self.repo.get(db, id).await?.ok_or(DomainError::NotFound("打印模板不存在".into()))?;
        self.repo.delete(db, id).await
    }

    async fn list(
        &self,
        db: PgExecutor<'_>,
        filter: PrintTemplateQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PrintTemplate>> {
        let (items, total) = self.repo.list(db, &filter, &page).await?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn set_default(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let template = self.repo.get(db, id).await?.ok_or(DomainError::NotFound("打印模板不存在".into()))?;
        self.repo.clear_default(db, &template.document_type).await?;
        self.repo.set_default(db, id).await
    }

    async fn render(
        &self,
        db: PgExecutor<'_>,
        template_id: i64,
        vars: RenderVars,
    ) -> Result<String> {
        let template = self.repo.get(db, template_id).await?.ok_or(DomainError::NotFound("打印模板不存在".into()))?;
        Self::render_template(&template.html_content, &vars)
    }

    async fn render_default(
        &self,
        db: PgExecutor<'_>,
        document_type: &str,
        vars: RenderVars,
    ) -> Result<String> {
        let template = self
            .repo
            .find_default(db, document_type)
            .await?;

        match template {
            Some(t) => Self::render_template(&t.html_content, &vars),
            None => Err(DomainError::NotFound(format!(
                "未找到文档类型 '{document_type}' 的打印模板"
            ))),
        }
    }
}

impl PrintTemplateServiceImpl {
    /// 用 minijinja 渲染模板。auto_escape=Html：模板字面 HTML 原样输出，
    /// 仅 `{{ }}` 输出的值做 HTML 实体转义（修复旧实现无转义的 XSS 隐患）。
    fn render_template(html_content: &str, vars: &RenderVars) -> Result<String> {
        let mut env = Environment::new();
        env.set_auto_escape_callback(|_| AutoEscape::Html);
        env.render_str(html_content, vars)
            .map_err(|e| DomainError::Internal(anyhow::anyhow!("打印模板渲染失败: {e}")))
    }

    /// 渲染任意模板字符串（不入库，编辑器实时预览用）。handler 直接调此静态方法。
    pub fn render_html(html_content: &str, vars: &RenderVars) -> Result<String> {
        Self::render_template(html_content, vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_chinese_var_and_loop_render() {
        // 验证 minijinja unicode feature：中文变量名 + 明细行循环 + loop.index
        let tpl = "<h1>{{ 客户全称 }}</h1>{% for item in 明细 %}<li>{{ loop.index }}.{{ item.产品名称 }}</li>{% endfor %}";
        let vars = json!({
            "客户全称": "测试客户",
            "明细": [{ "产品名称": "商品甲" }, { "产品名称": "商品乙" }]
        });
        let out = PrintTemplateServiceImpl::render_html(tpl, &vars).unwrap();
        assert!(out.contains("测试客户"), "中文单值变量应渲染: {out}");
        assert!(out.contains("1.商品甲"), "明细行循环+序号应渲染: {out}");
        assert!(out.contains("2.商品乙"), "第二行明细应渲染: {out}");
    }

    #[test]
    fn test_auto_escape_html() {
        // auto_escape=Html：{{ }} 输出的值做 HTML 转义（修复旧实现无转义的 XSS 隐患）
        let tpl = "<div>{{ 备注 }}</div>";
        let vars = json!({ "备注": "<script>alert(1)</script>" });
        let out = PrintTemplateServiceImpl::render_html(tpl, &vars).unwrap();
        assert!(out.contains("&lt;script&gt;"), "值应被转义: {out}");
        assert!(!out.contains("<script>alert"), "不应出现原始 script 标签: {out}");
    }
}
