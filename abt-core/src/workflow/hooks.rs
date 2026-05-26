//! WorkflowHook trait + HookRegistry

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;

use crate::workflow::model::event_type::HOOK_EXECUTED;
use crate::workflow::model::WorkflowInstance;
use crate::workflow::repo::WorkflowHistoryRepo;

/// WorkflowHook trait — 事务后异步执行的业务回调
#[async_trait]
pub trait WorkflowHook: Send + Sync {
    async fn on_approved(
        &self,
        pool: &PgPool,
        instance: &WorkflowInstance,
    ) -> Result<()>;

    async fn on_rejected(
        &self,
        pool: &PgPool,
        instance: &WorkflowInstance,
    ) -> Result<()>;
}

/// HookRegistry — 按 entity_type 注册 hook
pub struct HookRegistry {
    hooks: HashMap<String, Arc<dyn WorkflowHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    pub fn register(&mut self, entity_type: &str, hook: Arc<dyn WorkflowHook>) {
        self.hooks.insert(entity_type.to_string(), hook);
    }

    pub fn get(&self, entity_type: &str) -> Option<&Arc<dyn WorkflowHook>> {
        self.hooks.get(entity_type)
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

async fn record_hook_history(
    pool: &PgPool,
    instance_id: i64,
    event: &str,
    success: bool,
    error: Option<&str>,
    operator_id: Option<i64>,
    retry: bool,
) {
    let payload = match (success, error, retry) {
        (true, _, false) => json!({"success": true, "event": event}),
        (true, _, true) => json!({"success": true, "event": event, "retry": true}),
        (false, Some(e), false) => json!({"success": false, "event": event, "error": e}),
        (false, Some(e), true) => json!({"success": false, "event": event, "error": e, "retry": true}),
        (false, None, retry) => json!({"success": false, "event": event, "retry": retry}),
    };
    if let Ok(mut conn) = pool.acquire().await {
        let _ = WorkflowHistoryRepo::insert(
            conn.as_mut(),
            instance_id,
            None,
            None,
            HOOK_EXECUTED,
            operator_id,
            Some(payload),
        )
        .await;
    }
}

/// 异步触发 hook（在事务 commit 后调用）
pub async fn fire_hook(
    pool: Arc<PgPool>,
    hook_registry: Arc<HookRegistry>,
    instance: WorkflowInstance,
    event: &str,
) {
    let hook = match hook_registry.get(&instance.entity_type) {
        Some(h) => h,
        None => return,
    };

    let result = match event {
        "approved" => hook.on_approved(&pool, &instance).await,
        "rejected" => hook.on_rejected(&pool, &instance).await,
        _ => return,
    };

    match result {
        Err(e) => {
            tracing::error!(
                "workflow hook failed: instance_id={}, event={}, error={e:#}",
                instance.id,
                event
            );
            record_hook_history(&pool, instance.id, event, false, Some(&format!("{e:#}")), None, false)
                .await;
        }
        Ok(()) => {
            record_hook_history(&pool, instance.id, event, true, None, None, false).await;
        }
    }
}

/// RetryFailedHook：查找最近的失败 hook 记录并重新执行
pub async fn retry_failed_hook(
    pool: &PgPool,
    hook_registry: &HookRegistry,
    instance_id: i64,
    operator_id: i64,
) -> Result<()> {
    let failed_record = WorkflowHistoryRepo::find_latest_failed_hook(pool, instance_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no failed hook found for instance {instance_id}"))?;

    let event = failed_record
        .payload
        .as_ref()
        .and_then(|p| p.get("event"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let instance = crate::workflow::repo::WorkflowInstanceRepo::find_by_id(pool, instance_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

    let hook = hook_registry
        .get(&instance.entity_type)
        .ok_or_else(|| anyhow::anyhow!("no hook registered for entity type: {}", instance.entity_type))?;

    let result = match event {
        "approved" => hook.on_approved(pool, &instance).await,
        "rejected" => hook.on_rejected(pool, &instance).await,
        _ => bail!("unknown hook event: {event}"),
    };

    match result {
        Ok(()) => {
            record_hook_history(pool, instance_id, event, true, None, Some(operator_id), true).await;
        }
        Err(e) => {
            let err_str = format!("{e:#}");
            record_hook_history(pool, instance_id, event, false, Some(&err_str), Some(operator_id), true)
                .await;
            bail!("hook retry failed: {e:#}");
        }
    }

    Ok(())
}
