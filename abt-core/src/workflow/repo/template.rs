//! 工作流模板数据访问层

use sqlx::PgPool;
use crate::shared::types::Result;

use crate::workflow::model::WorkflowTemplate;

pub struct WorkflowTemplateRepo;

impl WorkflowTemplateRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        entity_type: &str,
        name: &str,
        graph: Option<serde_json::Value>,
        trigger_event: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO workflow_templates (entity_type, name, graph, trigger_event) VALUES ($1, $2, $3::jsonb, $4) RETURNING id",
        )
        .bind(entity_type)
        .bind(name)
        .bind(graph)
        .bind(trigger_event)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        name: Option<&str>,
        graph: Option<serde_json::Value>,
        trigger_event: Option<Option<&str>>,
    ) -> Result<()> {
        if let Some(n) = name {
            sqlx::query(
                "UPDATE workflow_templates SET name = $1, updated_at = NOW() WHERE id = $2 AND status = 'draft'",
            )
            .bind(n)
            .bind(id)
            .execute(&mut *executor)
            .await?;
        }
        if let Some(g) = graph {
            sqlx::query(
                "UPDATE workflow_templates SET graph = $1::jsonb, updated_at = NOW() WHERE id = $2 AND status = 'draft'",
            )
            .bind(g)
            .bind(id)
            .execute(&mut *executor)
            .await?;
        }
        if let Some(te) = trigger_event {
            sqlx::query(
                "UPDATE workflow_templates SET trigger_event = $1, updated_at = NOW() WHERE id = $2 AND status = 'draft'",
            )
            .bind(te)
            .bind(id)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<WorkflowTemplate>> {
        let row = sqlx::query_as::<_, WorkflowTemplate>(
            "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_active_by_entity_type(
        pool: &PgPool,
        entity_type: &str,
    ) -> Result<Option<WorkflowTemplate>> {
        let row = sqlx::query_as::<_, WorkflowTemplate>(
            "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE entity_type = $1 AND status = 'active' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(entity_type)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_entity_type(
        pool: &PgPool,
        entity_type: &str,
    ) -> Result<Vec<WorkflowTemplate>> {
        let rows = if entity_type.is_empty() {
            sqlx::query_as::<_, WorkflowTemplate>(
                "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE deleted_at IS NULL ORDER BY version DESC",
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, WorkflowTemplate>(
                "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE entity_type = $1 AND deleted_at IS NULL ORDER BY version DESC",
            )
            .bind(entity_type)
            .fetch_all(pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn publish(executor: &mut sqlx::postgres::PgConnection, id: i64, graph_checksum: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE workflow_templates SET status = 'active', graph_checksum = $1, updated_at = NOW() WHERE id = $2 AND status = 'draft'",
        )
        .bind(graph_checksum)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn archive(executor: &mut sqlx::postgres::PgConnection, id: i64) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE workflow_templates SET status = 'archived', updated_at = NOW() WHERE id = $1 AND status = 'active'",
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_active(pool: &PgPool) -> Result<Vec<WorkflowTemplate>> {
        let rows = sqlx::query_as::<_, WorkflowTemplate>(
            "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE status = 'active' AND deleted_at IS NULL",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_active_by_trigger(
        pool: &PgPool,
        trigger_event: &str,
    ) -> Result<Option<WorkflowTemplate>> {
        let row = sqlx::query_as::<_, WorkflowTemplate>(
            "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates WHERE trigger_event = $1 AND status = 'active' AND deleted_at IS NULL LIMIT 1",
        )
        .bind(trigger_event)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
