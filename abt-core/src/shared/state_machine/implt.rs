use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::instrument;

use super::model::{EntityStateLog, StateDefinitionInput, StateLogEntry, TransitionDefInput};
use super::repo::StateMachineRepo;
use super::service::StateMachineService;
use crate::shared::enums::SideEffect;
use crate::shared::types::PgExecutor;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::new_domain_event_bus;
use crate::shared::types::Result;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct StateMachineServiceImpl {
    pool: PgPool,
}

impl StateMachineServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StateMachineService for StateMachineServiceImpl {
    #[instrument(skip(self, _ctx, states, transitions))]
    async fn configure(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: &str,
        states: Vec<StateDefinitionInput>,
        transitions: Vec<TransitionDefInput>,
    ) -> Result<()> {
        StateMachineRepo::delete_transitions(db, entity_type).await?;
        StateMachineRepo::delete_definitions(db, entity_type).await?;

        for state in &states {
            StateMachineRepo::insert_definition(db, entity_type, state).await?;
        }

        for tr in &transitions {
            StateMachineRepo::insert_transition(db, entity_type, tr).await?;
        }

        Ok(())
    }

    #[instrument(skip(self, ctx), fields(entity_type = %entity_type, entity_id = %entity_id, to_state = %to_state))]
    async fn transition(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
        to_state: &str,
        remark: Option<&str>,
    ) -> Result<EntityStateLog> {
        // Step 1: 查询当前状态（None 表示新实体，使用空字符串）
        let from_state = match StateMachineRepo::get_current_state(db, entity_type, entity_id).await {
            Ok(state) => state,
            Err(e) => return Err(e),
        };

        // Step 2: 匹配转换规则
        let from = from_state.as_deref().unwrap_or("");
        let transition_def = StateMachineRepo::find_transition_def(db, entity_type, from, to_state)
            .await?
            .ok_or_else(|| DomainError::InvalidStateTransition {
                from: from.to_string(),
                to: to_state.to_string(),
            })?;

        // Step 3: 插入 EntityStateLog
        let state_log = StateMachineRepo::insert_state_log(
            db,
            &StateLogEntry {
                entity_type,
                entity_id,
                from_state: from_state.as_deref(),
                to_state,
                transition_id: transition_def.id,
                operator_id: ctx.operator_id,
                remark,
            },
        )
        .await?;

        // Step 4: 执行 side_effects
        for effect in &transition_def.side_effects {
            match effect {
                SideEffect::PublishEvent {
                    event_type,
                    payload_template,
                } => {
                    if let Some(et) = event_type
                        .parse::<i16>()
                        .ok()
                        .and_then(DomainEventType::from_i16)
                    {
                        let req = EventPublishRequest {
                            event_type: et,
                            aggregate_type: entity_type.to_string(),
                            aggregate_id: entity_id,
                            payload: payload_template.clone(),
                            idempotency_key: Some(format!(
                                "sm:{}:{}:{}",
                                entity_type,
                                entity_id,
                                et.as_i16()
                            )),
                        };
                        new_domain_event_bus(self.pool.clone())
                        .publish(ctx, db, req)
                        .await?;
                    }
                }
                SideEffect::Notify { .. } => {}
                SideEffect::TriggerWorkflow { .. } => {}
                SideEffect::UpdateField { .. } => {}
            }
        }

        Ok(state_log)
    }

    #[instrument(skip(self, _ctx))]
    async fn get_current_state(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<String> {
        match StateMachineRepo::get_current_state(db, entity_type, entity_id).await {
            Ok(Some(state)) => Ok(state),
            Ok(None) => {
                StateMachineRepo::get_initial_state(db, entity_type)
                    .await?
                    .ok_or_else(|| {
                        DomainError::not_found(format!(
                            "initial state for entity_type '{entity_type}'"
                        ))
                    })
            }
            Err(e) => Err(e),
        }
    }

    #[instrument(skip(self, _ctx))]
    async fn get_allowed_transitions(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: &str,
        state: &str,
    ) -> Result<Vec<String>> {
        StateMachineRepo::get_allowed_transitions(db, entity_type, state).await
    }

    #[instrument(skip(self, _ctx))]
    async fn get_state_history(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<EntityStateLog>> {
        let params = PageParams::new(page, page_size);

        let total = StateMachineRepo::count_state_history(db, entity_type, entity_id).await?;
        let items = StateMachineRepo::query_state_history(
            db,
            entity_type,
            entity_id,
            params.page_size as i64,
            params.offset() as i64,
        )
        .await?;

        Ok(PaginatedResult::new(
            items,
            total as u64,
            params.page,
            params.page_size,
        ))
    }
}
