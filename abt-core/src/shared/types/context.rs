use crate::shared::types::PgExecutor;

use super::pagination::DataScope;

/// 服务调用上下文 — 包装事务执行器 + 操作元数据
pub struct ServiceContext<'a> {
    pub executor: PgExecutor<'a>,
    pub operator_id: i64,
    pub department_id: Option<i64>,
    pub data_scope: DataScope,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
}

impl<'a> ServiceContext<'a> {
    pub fn new(executor: PgExecutor<'a>, operator_id: i64) -> Self {
        Self {
            executor,
            operator_id,
            department_id: None,
            data_scope: DataScope::All,
            trace_id: None,
            request_id: None,
        }
    }

    pub fn with_department(mut self, department_id: i64) -> Self {
        self.department_id = Some(department_id);
        self
    }

    pub fn with_data_scope(mut self, scope: DataScope) -> Self {
        self.data_scope = scope;
        self
    }

    pub fn with_trace_id(mut self, trace_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// 系统级上下文 — 用于定时任务、后台进程等无用户操作场景
    pub fn system(executor: PgExecutor<'a>) -> Self {
        Self {
            executor,
            operator_id: 0,
            department_id: None,
            data_scope: DataScope::All,
            trace_id: Some("system".to_string()),
            request_id: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    /// 从现有 context 中 reborrow executor，避免手动重复构造
    pub fn reborrow(&mut self) -> ServiceContext<'_> {
        ServiceContext {
            executor: &mut *self.executor,
            operator_id: self.operator_id,
            department_id: self.department_id,
            data_scope: self.data_scope,
            trace_id: self.trace_id.clone(),
            request_id: self.request_id.clone(),
        }
    }
}
