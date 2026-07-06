use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::PageParams;

const COLUMNS: &str = "id, name, document_type, description, html_content, is_default, created_at, updated_at";

pub struct PrintTemplateRepo;

impl PrintTemplateRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, req: &CreatePrintTemplateReq) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO print_templates (name, document_type, description, html_content, is_default)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"#,
        )
        .bind(&req.name)
        .bind(&req.document_type)
        .bind(&req.description)
        .bind(&req.html_content)
        .bind(req.is_default)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn get(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<PrintTemplate>> {
        let row = sqlx::query_as::<sqlx::Postgres, PrintTemplate>(
            sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM print_templates WHERE id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn update(&self, executor: PgExecutor<'_>, id: i64, req: &UpdatePrintTemplateReq) -> Result<()> {
        let mut sets: Vec<String> = Vec::new();
        let mut idx: u32 = 2;

        if req.name.is_some() {
            sets.push(format!("name = ${idx}"));
            idx += 1;
        }
        if req.document_type.is_some() {
            sets.push(format!("document_type = ${idx}"));
            idx += 1;
        }
        if req.description.is_some() {
            sets.push(format!("description = ${idx}"));
            idx += 1;
        }
        if req.html_content.is_some() {
            sets.push(format!("html_content = ${idx}"));
            idx += 1;
        }
        if req.is_default.is_some() {
            sets.push(format!("is_default = ${idx}"));
            idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let query = format!("UPDATE print_templates SET {} WHERE id = $1 AND deleted_at IS NULL", sets.join(", "));

        let mut q = sqlx::query(sqlx::AssertSqlSafe(query)).bind(id);

        if let Some(v) = &req.name {
            q = q.bind(v);
        }
        if let Some(v) = &req.document_type {
            q = q.bind(v);
        }
        if let Some(v) = &req.description {
            q = q.bind(v);
        }
        if let Some(v) = &req.html_content {
            q = q.bind(v);
        }
        if let Some(v) = req.is_default {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE print_templates SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn list(
        &self,
        executor: PgExecutor<'_>,
        filter: &PrintTemplateQuery,
        page: &PageParams,
    ) -> Result<(Vec<PrintTemplate>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx: u32 = 1;

        if filter.document_type.is_some() {
            conditions.push(format!("document_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.keyword.is_some() {
            conditions.push(format!("(name ILIKE ${param_idx} OR description ILIKE ${param_idx})"));
            param_idx += 1;
        }

        let where_clause = conditions.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) FROM print_templates WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(dt) = &filter.document_type {
            count_q = count_q.bind(dt);
        }
        if let Some(kw) = &filter.keyword {
            count_q = count_q.bind(format!("%{kw}%"));
        }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data
        let offset = ((page.page.max(1) - 1) * page.page_size) as i64;
        let data_sql = format!(
            "SELECT {COLUMNS} FROM print_templates WHERE {where_clause} ORDER BY created_at DESC LIMIT ${param_idx} OFFSET ${param_idx1}",
            param_idx = param_idx,
            param_idx1 = param_idx + 1,
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, PrintTemplate>(sqlx::AssertSqlSafe(data_sql));
        if let Some(dt) = &filter.document_type {
            data_q = data_q.bind(dt);
        }
        if let Some(kw) = &filter.keyword {
            data_q = data_q.bind(format!("%{kw}%"));
        }
        data_q = data_q.bind(page.page_size as i64).bind(offset);

        let rows = data_q.fetch_all(executor).await?;
        Ok((rows, total))
    }

    pub async fn clear_default(&self, executor: PgExecutor<'_>, document_type: &str) -> Result<()> {
        sqlx::query("UPDATE print_templates SET is_default = FALSE WHERE document_type = $1 AND deleted_at IS NULL")
            .bind(document_type)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn set_default(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE print_templates SET is_default = TRUE WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_default(
        &self,
        executor: PgExecutor<'_>,
        document_type: &str,
    ) -> Result<Option<PrintTemplate>> {
        let row = sqlx::query_as::<sqlx::Postgres, PrintTemplate>(
            sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM print_templates WHERE document_type = $1 AND is_default = TRUE AND deleted_at IS NULL")),
        )
        .bind(document_type)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn list_by_document_type(
        &self,
        executor: PgExecutor<'_>,
        document_type: &str,
    ) -> Result<Vec<PrintTemplate>> {
        let rows = sqlx::query_as::<sqlx::Postgres, PrintTemplate>(
            sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM print_templates WHERE document_type = $1 AND deleted_at IS NULL ORDER BY is_default DESC, created_at DESC")),
        )
        .bind(document_type)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
