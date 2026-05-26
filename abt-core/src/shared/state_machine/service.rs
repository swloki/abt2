use async_trait::async_trait;

use super::model::{EntityStateLog, StateDefinitionInput, TransitionDefInput};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

/// 状态机服务 — 管理实体的状态定义、转换规则和状态变更
#[async_trait]
pub trait StateMachineService: Send + Sync {
    /// 批量配置状态定义和转换规则（启动时一次性或按需）
    async fn configure(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        states: Vec<StateDefinitionInput>,
        transitions: Vec<TransitionDefInput>,
    ) -> Result<()>;

    /// 执行状态转换 — 五步校验
    async fn transition(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        to_state: &str,
        remark: Option<&str>,
    ) -> Result<EntityStateLog>;

    /// 获取当前状态
    async fn get_current_state(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<String>;

    /// 获取允许的目标状态列表
    async fn get_allowed_transitions(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        state: &str,
    ) -> Result<Vec<String>>;

    /// 分页查询状态历史
    async fn get_state_history(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<EntityStateLog>>;
}
