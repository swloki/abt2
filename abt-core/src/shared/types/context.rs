use super::pagination::DataScope;

/// 服务调用上下文 — 纯操作元数据，不持有数据库连接
/// 连接由 PgExecutor 参数单独传递
#[derive(Clone)]
pub struct ServiceContext {
    pub operator_id: i64,
    pub department_id: Option<i64>,
    pub data_scope: DataScope,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
}

impl ServiceContext {
    pub fn new(operator_id: i64) -> Self {
        Self {
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
    pub fn system() -> Self {
        Self {
            operator_id: 0,
            department_id: None,
            data_scope: DataScope::All,
            trace_id: Some("system".to_string()),
            request_id: Some(uuid::Uuid::new_v4().to_string()),
        }
    }
}
