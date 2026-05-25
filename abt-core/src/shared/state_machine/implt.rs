use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Row};
use tracing::instrument;

use super::model::{
    EntityStateLog, StateDefinitionInput, StateTransitionDef, TransitionDefInput,
};
use super::service::StateMachineService;
use crate::shared::enums::SideEffect;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct StateMachineServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    event_bus: Arc<dyn DomainEventBus>,
}

impl StateMachineServiceImpl {
    pub fn new(pool: Arc<PgPool>, event_bus: Arc<dyn DomainEventBus>) -> Self {
        Self { pool, event_bus }
    }
}

#[async_trait]
impl StateMachineService for StateMachineServiceImpl {
    #[instrument(skip(self, ctx, states, transitions))]
    async fn configure(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        states: Vec<StateDefinitionInput>,
        transitions: Vec<TransitionDefInput>,
    ) -> Result<(), DomainError> {
        let executor = &mut *ctx.executor;

        sqlx::query("DELETE FROM state_transition_defs WHERE entity_type = $1")
            .bind(entity_type)
            .execute(&mut *executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        sqlx::query("DELETE FROM state_definitions WHERE entity_type = $1")
            .bind(entity_type)
            .execute(&mut *executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for state in &states {
            sqlx::query(
                r#"
                INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(entity_type)
            .bind(&state.state_name)
            .bind(&state.label)
            .bind(state.is_initial)
            .bind(state.is_final)
            .execute(&mut *executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        for tr in &transitions {
            let side_effects_json = serde_json::to_value(&tr.side_effects)
                .unwrap_or(JsonValue::Null);

            sqlx::query(
                r#"
                INSERT INTO state_transition_defs
                    (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(entity_type)
            .bind(&tr.from_state)
            .bind(&tr.to_state)
            .bind(tr.trigger_event.map(|e| e.as_i16()))
            .bind(&tr.guard_condition)
            .bind(&side_effects_json)
            .bind(tr.sort_order)
            .execute(&mut *executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(())
    }

    #[instrument(skip(self, ctx), fields(entity_type = %entity_type, entity_id = %entity_id, to_state = %to_state))]
    async fn transition(
        &self,
        mut ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        to_state: &str,
        remark: Option<&str>,
    ) -> Result<EntityStateLog, DomainError> {
        // Step 1: 查询当前状态（NotFound 表示新实体，使用空字符串）
        let from_state = match self
            .get_current_state(ctx.reborrow(), entity_type, entity_id)
            .await
        {
            Ok(state) => Some(state),
            Err(DomainError::NotFound(_)) => None,
            Err(e) => return Err(e),
        };

        // Step 2: 匹配转换规则
        let from = from_state.as_deref().unwrap_or("");
        let row = sqlx::query(
            r#"
            SELECT id, entity_type, from_state, to_state, trigger_event,
                   guard_condition, side_effects, sort_order
            FROM state_transition_defs
            WHERE entity_type = $1 AND from_state = $2 AND to_state = $3
            ORDER BY sort_order
            LIMIT 1
            "#,
        )
        .bind(entity_type)
        .bind(from)
        .bind(to_state)
        .fetch_optional(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let transition_def: StateTransitionDef = match row {
            Some(r) => StateTransitionDef::try_from(&r)
                .map_err(|e| DomainError::Internal(e.into()))?,
            None => {
                return Err(DomainError::InvalidStateTransition {
                    from: from.to_string(),
                    to: to_state.to_string(),
                })
            }
        };

        // Step 3: 插入 EntityStateLog
        let log_row = sqlx::query(
            r#"
            INSERT INTO entity_state_logs
                (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, entity_type, entity_id, from_state, to_state, transition_id,
                      operator_id, remark, created_at
            "#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(&from_state)
        .bind(to_state)
        .bind(transition_def.id)
        .bind(ctx.operator_id)
        .bind(remark)
        .fetch_one(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let state_log = EntityStateLog::from_row(&log_row)
            .map_err(|e| DomainError::Internal(e.into()))?;

        // Step 4: 执行 side_effects
        for effect in &transition_def.side_effects {
            match effect {
                SideEffect::PublishEvent {
                    event_type,
                    payload_template,
                } => {
                    if let Some(et) = event_type.parse::<i16>().ok().and_then(DomainEventType::from_i16) {
                        let req = EventPublishRequest {
                            event_type: et,
                            aggregate_type: entity_type.to_string(),
                            aggregate_id: entity_id,
                            payload: payload_template.clone(),
                            idempotency_key: Some(format!("sm:{}:{}:{}", entity_type, entity_id, et.as_i16())),
                        };
                        self.event_bus.publish(ctx.reborrow(), req).await?;
                    }
                }
                SideEffect::Notify { .. } => {}
                SideEffect::TriggerWorkflow { .. } => {}
                SideEffect::UpdateField { .. } => {}
            }
        }

        Ok(state_log)
    }

    #[instrument(skip(self, ctx))]
    async fn get_current_state(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<String, DomainError> {
        let row = sqlx::query(
            r#"
            SELECT to_state
            FROM entity_state_logs
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_optional(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        match row {
            Some(r) => Ok(r.try_get::<String, _>("to_state")
                .map_err(|e| DomainError::Internal(e.into()))?),
            None => {
                let init_row = sqlx::query(
                    r#"
                    SELECT state_name
                    FROM state_definitions
                    WHERE entity_type = $1 AND is_initial = true
                    LIMIT 1
                    "#,
                )
                .bind(entity_type)
                .fetch_optional(&mut *ctx.executor)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

                init_row
                    .map(|r| {
                        r.try_get::<String, _>("state_name")
                            .map_err(|e| DomainError::Internal(e.into()))
                    })
                    .transpose()?
                    .ok_or_else(|| {
                        DomainError::not_found(format!(
                            "initial state for entity_type '{entity_type}'"
                        ))
                    })
            }
        }
    }

    #[instrument(skip(self, ctx))]
    async fn get_allowed_transitions(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        state: &str,
    ) -> Result<Vec<String>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT to_state
            FROM state_transition_defs
            WHERE entity_type = $1 AND from_state = $2
            ORDER BY sort_order
            "#,
        )
        .bind(entity_type)
        .bind(state)
        .fetch_all(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        rows.iter()
            .map(|r| r.try_get::<String, _>("to_state").map_err(|e| DomainError::Internal(e.into())))
            .collect()
    }

    #[instrument(skip(self, ctx))]
    async fn get_state_history(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<EntityStateLog>, DomainError> {
        let params = PageParams::new(page, page_size);

        let count_row = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM entity_state_logs WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_one(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 = count_row.try_get("cnt").map_err(|e| DomainError::Internal(e.into()))?;

        let rows = sqlx::query(
            r#"
            SELECT id, entity_type, entity_id, from_state, to_state,
                   transition_id, operator_id, remark, created_at
            FROM entity_state_logs
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(params.page_size as i64)
        .bind(params.offset() as i64)
        .fetch_all(&mut *ctx.executor)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let items: Vec<EntityStateLog> = rows
            .iter()
            .map(|r| EntityStateLog::from_row(r).map_err(|e| DomainError::Internal(e.into())))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PaginatedResult::new(
            items,
            total as u64,
            params.page,
            params.page_size,
        ))
    }
}
