//! 工作流实例数据访问层

use sqlx::PgPool;
use crate::shared::types::RepoResult;

use crate::workflow::model::WorkflowInstance;

pub struct InstanceInsertParams<'a> {
    pub template_id: i64,
    pub template_version: Option<i32>,
    pub entity_type: &'a str,
    pub entity_id: i64,
    pub frozen_graph: serde_json::Value,
    pub context: serde_json::Value,
    pub initiator_id: i64,
}

pub struct WorkflowInstanceRepo;

impl WorkflowInstanceRepo {
    pub async fn insert(executor: &mut sqlx::postgres::PgConnection, p: &InstanceInsertParams<'_>) -> RepoResult<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO workflow_instances (template_id, template_version, entity_type, entity_id, frozen_graph, context, initiator_id) VALUES ($1, $2, $3, $4, $5::jsonb, $6::jsonb, $7) RETURNING id",
        )
        .bind(p.template_id)
        .bind(p.template_version)
        .bind(p.entity_type)
        .bind(p.entity_id)
        .bind(&p.frozen_graph)
        .bind(&p.context)
        .bind(p.initiator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_by_id(pool: &PgPool, id: i64) -> RepoResult<Option<WorkflowInstance>> {
        let row = sqlx::query_as::<_, WorkflowInstance>(
            "SELECT id, template_id, template_version, entity_type, entity_id, status, frozen_graph, context, suspended_reason, initiator_id, created_at, updated_at, last_advanced_at, completed_at FROM workflow_instances WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_for_update(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<Option<WorkflowInstance>> {
        let row = sqlx::query_as::<_, WorkflowInstance>(
            "SELECT id, template_id, template_version, entity_type, entity_id, status, frozen_graph, context, suspended_reason, initiator_id, created_at, updated_at, last_advanced_at, completed_at FROM workflow_instances WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn update_status(executor: &mut sqlx::postgres::PgConnection, id: i64, status: &str) -> RepoResult<()> {
        let completed_at = if matches!(status, "completed" | "rejected" | "cancelled" | "terminated") {
            Some(chrono::Utc::now())
        } else {
            None
        };
        sqlx::query(
            "UPDATE workflow_instances SET status = $1, updated_at = NOW(), completed_at = COALESCE($2, completed_at), last_advanced_at = NOW() WHERE id = $3",
        )
        .bind(status)
        .bind(completed_at)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_suspended_reason(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        reason: Option<serde_json::Value>,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE workflow_instances SET suspended_reason = $1::jsonb, updated_at = NOW() WHERE id = $2",
        )
        .bind(reason)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_by_entity(
        pool: &PgPool,
        entity_type: &str,
        entity_id: i64,
    ) -> RepoResult<Vec<WorkflowInstance>> {
        let rows = sqlx::query_as::<_, WorkflowInstance>(
            "SELECT id, template_id, template_version, entity_type, entity_id, status, frozen_graph, context, suspended_reason, initiator_id, created_at, updated_at, last_advanced_at, completed_at FROM workflow_instances WHERE entity_type = $1 AND entity_id = $2 ORDER BY created_at DESC",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

}
