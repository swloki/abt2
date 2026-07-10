use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 工艺路线实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Routing {
    pub id: i64,
    pub code: String,
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
    #[sqlx(default)]
    pub process_name: Option<String>,
    // migration 045 新增工序属性
    #[sqlx(default)]
    pub work_center_id: Option<i64>,
    #[sqlx(default)]
    pub standard_time: Option<Decimal>,
    #[sqlx(default)]
    pub standard_cost: Option<Decimal>,
    #[sqlx(default)]
    pub allowed_loss_rate: Option<Decimal>,
    #[sqlx(default)]
    pub is_outsourced: bool,
    #[sqlx(default)]
    pub is_inspection_point: bool,
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
    /// 关联产品名称（JOIN products.pdt_name，LEFT JOIN 可能为 None）
    #[sqlx(default)]
    pub product_name: Option<String>,
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
#[derive(Debug, Clone, Default)]
pub struct RoutingStepInput {
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
    // migration 045 新增工序属性
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub allowed_loss_rate: Option<Decimal>,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
}

/// 工艺路线查询
#[derive(Debug, Clone, Default)]
pub struct RoutingQuery {
    pub keyword: Option<String>,
    /// 按关联 BOM 的产品编码/名称过滤（反查：某 BOM 关联了哪些 routing）
    pub bom_keyword: Option<String>,
}

/// 工艺路线详情（含步骤）
#[derive(Debug, Clone)]
pub struct RoutingDetail {
    pub routing: Routing,
    pub steps: Vec<RoutingStep>,
}
