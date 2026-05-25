use chrono::{DateTime, Utc};

/// 工艺路线实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Routing {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 工艺路线步骤
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoutingStep {
    pub id: i64,
    pub routing_id: i64,
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// BOM-工艺路线关联
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomRouting {
    pub id: i64,
    pub product_code: String,
    pub routing_id: i64,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 创建工艺路线请求
#[derive(Debug, Clone)]
pub struct CreateRoutingReq {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<RoutingStepInput>,
}

/// 更新工艺路线请求
#[derive(Debug, Clone, Default)]
pub struct UpdateRoutingReq {
    pub name: Option<String>,
    pub description: Option<String>,
    pub steps: Option<Vec<RoutingStepInput>>,
}

/// 工序步骤输入
#[derive(Debug, Clone)]
pub struct RoutingStepInput {
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
}

/// 工艺路线查询
#[derive(Debug, Clone, Default)]
pub struct RoutingQuery {
    pub keyword: Option<String>,
}

/// 工艺路线详情（含步骤）
#[derive(Debug, Clone)]
pub struct RoutingDetail {
    pub routing: Routing,
    pub steps: Vec<RoutingStep>,
}
