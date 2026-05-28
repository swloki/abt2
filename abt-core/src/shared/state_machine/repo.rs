use sqlx::{FromRow, Row};

use super::model::{EntityStateLog, StateDefinitionInput, StateLogEntry, StateTransitionDef, TransitionDefInput};
use crate::shared::types::{DomainError, PgExecutor, Result};

pub struct StateMachineRepo;

impl StateMachineRepo {
    // ── configure 相关 ──────────────────────────────────────────

    pub async fn delete_transitions(db: PgExecutor<'_>, entity_type: &str) -> Result<()> {
        sqlx::query("DELETE FROM state_transition_defs WHERE entity_type = $1")
            .bind(entity_type)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    pub async fn delete_definitions(db: PgExecutor<'_>, entity_type: &str) -> Result<()> {
        sqlx::query("DELETE FROM state_definitions WHERE entity_type = $1")
            .bind(entity_type)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    pub async fn insert_definition(
        db: PgExecutor<'_>,
        entity_type: &str,
        input: &StateDefinitionInput,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(entity_type)
        .bind(&input.state_name)
        .bind(&input.label)
        .bind(input.is_initial)
        .bind(input.is_final)
        .execute(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    pub async fn insert_transition(
        db: PgExecutor<'_>,
        entity_type: &str,
        input: &TransitionDefInput,
    ) -> Result<()> {
        let side_effects_json =
            serde_json::to_value(&input.side_effects).unwrap_or(serde_json::Value::Null);

        sqlx::query(
            r#"
            INSERT INTO state_transition_defs
                (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(entity_type)
        .bind(&input.from_state)
        .bind(&input.to_state)
        .bind(input.trigger_event.map(|e| e.as_i16()))
        .bind(&input.guard_condition)
        .bind(&side_effects_json)
        .bind(input.sort_order)
        .execute(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    // ── transition 相关 ─────────────────────────────────────────

    pub async fn find_transition_def(
        db: PgExecutor<'_>,
        entity_type: &str,
        from_state: &str,
        to_state: &str,
    ) -> Result<Option<StateTransitionDef>> {
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
        .bind(from_state)
        .bind(to_state)
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        row.map(|r| StateTransitionDef::try_from(&r).map_err(|e| DomainError::Internal(e.into())))
            .transpose()
    }

    pub async fn insert_state_log(
        db: PgExecutor<'_>,
        entry: &StateLogEntry<'_>,
    ) -> Result<EntityStateLog> {
        let row = sqlx::query(
            r#"
            INSERT INTO entity_state_logs
                (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, entity_type, entity_id, from_state, to_state, transition_id,
                      operator_id, remark, created_at
            "#,
        )
        .bind(entry.entity_type)
        .bind(entry.entity_id)
        .bind(entry.from_state)
        .bind(entry.to_state)
        .bind(entry.transition_id)
        .bind(entry.operator_id)
        .bind(entry.remark)
        .fetch_one(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        EntityStateLog::from_row(&row).map_err(|e| DomainError::Internal(e.into()))
    }

    // ── 查询相关 ────────────────────────────────────────────────

    pub async fn get_current_state(
        db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<String>> {
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
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        match row {
            Some(r) => Ok(Some(
                r.try_get::<String, _>("to_state")
                    .map_err(|e| DomainError::Internal(e.into()))?,
            )),
            None => Ok(None),
        }
    }

    pub async fn get_initial_state(
        db: PgExecutor<'_>,
        entity_type: &str,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT state_name
            FROM state_definitions
            WHERE entity_type = $1 AND is_initial = true
            LIMIT 1
            "#,
        )
        .bind(entity_type)
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        match row {
            Some(r) => Ok(Some(
                r.try_get::<String, _>("state_name")
                    .map_err(|e| DomainError::Internal(e.into()))?,
            )),
            None => Ok(None),
        }
    }

    pub async fn get_allowed_transitions(
        db: PgExecutor<'_>,
        entity_type: &str,
        state: &str,
    ) -> Result<Vec<String>> {
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
        .fetch_all(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        rows.iter()
            .map(|r| {
                r.try_get::<String, _>("to_state")
                    .map_err(|e| DomainError::Internal(e.into()))
            })
            .collect()
    }

    pub async fn count_state_history(
        db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<i64> {
        let count_row = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM entity_state_logs WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_one(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        count_row
            .try_get("cnt")
            .map_err(|e| DomainError::Internal(e.into()))
    }

    pub async fn query_state_history(
        db: PgExecutor<'_>,
        entity_type: &str,
        entity_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EntityStateLog>> {
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
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        rows.iter()
            .map(|r| EntityStateLog::from_row(r).map_err(|e| DomainError::Internal(e.into())))
            .collect()
    }
}
