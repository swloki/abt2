//! BOM (Bill of Materials) 数据模型
//!
//! 包含 BOM 实体及其相关的查询参数和详情结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// BOM 状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BomStatus {
    #[default]
    Draft,
    Published,
}

impl BomStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BomStatus::Draft => "draft",
            BomStatus::Published => "published",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        match s {
            "draft" => Ok(BomStatus::Draft),
            "published" => Ok(BomStatus::Published),
            other => anyhow::bail!("invalid BomStatus: {}", other),
        }
    }
}

impl std::fmt::Display for BomStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Bom {
    /// 检查用户是否有权访问此 BOM
    /// - 已发布 → 放行
    /// - 草稿 + 是创建者 → 放行
    /// - 否则 → Err（fail-closed: created_by 为 None 时拒绝）
    /// `reveal_existence`: false 用于读操作（NotFound），true 用于写操作（PermissionDenied）
    pub fn require_creator_or_published(&self, user_id: i64, reveal_existence: bool) -> Result<(), anyhow::Error> {
        if self.status == BomStatus::Published {
            return Ok(());
        }
        if self.created_by == Some(user_id) {
            return Ok(());
        }
        if reveal_existence {
            anyhow::bail!("Permission denied: only the creator can modify a draft BOM");
        } else {
            anyhow::bail!("BOM not found");
        }
    }
}

/// BOM 实体
#[derive(Default, Debug, Serialize)]
pub struct Bom {
    pub bom_id: i64,
    pub bom_name: String,
    pub create_at: DateTime<Utc>,
    pub update_at: Option<DateTime<Utc>>,
    pub bom_detail: BomDetail,
    pub bom_category_id: Option<i64>,
    pub status: BomStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
}

impl<'r> FromRow<'r, PgRow> for Bom {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let bom_id: i64 = row.try_get("bom_id")?;
        let bom_name: String = row.try_get("bom_name")?;
        let create_at: DateTime<Utc> = row.try_get("create_at")?;
        let update_at: Option<DateTime<Utc>> = row.try_get("update_at")?;
        let bom_category_id: Option<i64> = row.try_get("bom_category_id")?;

        let status_str: String = row.try_get("status")?;
        let status = BomStatus::from_str(&status_str).unwrap_or(BomStatus::Draft);

        let published_at: Option<DateTime<Utc>> = row.try_get("published_at")?;
        let created_by: Option<i64> = row.try_get("created_by")?;

        Ok(Bom {
            bom_id,
            bom_name,
            create_at,
            update_at,
            bom_detail: BomDetail::default(),
            bom_category_id,
            status,
            published_at,
            created_by,
        })
    }
}

/// BOM 详情（节点从 bom_nodes 表加载）
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BomDetail {
    pub nodes: Vec<BomNode>,
}

/// BOM 节点
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BomNode {
    /// 节点 ID
    pub id: i64,
    /// 产品 ID
    pub product_id: i64,
    /// 产品编码（用于出库）
    pub product_code: Option<String>,
    /// 数量
    pub quantity: f64,
    /// 父节点 ID（0 表示根节点）
    pub parent_id: i64,
    /// 损耗率
    pub loss_rate: f64,
    /// 排序顺序
    pub order: i32,
    /// 单位
    pub unit: Option<String>,
    /// 备注
    pub remark: Option<String>,
    /// 位置
    pub position: Option<String>,
    /// 工作中心
    pub work_center: Option<String>,
    /// 物料属性
    pub properties: Option<String>,
}

// ============================================================================
// 查询参数
// ============================================================================

/// BOM 查询参数
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BomQuery {
    /// BOM 名称（模糊匹配）
    pub bom_name: Option<String>,
    /// 创建者（模糊匹配）
    pub create_by: Option<String>,
    /// 删除 ID（用于删除操作）
    #[serde(default, deserialize_with = "deserialize_null_i64")]
    pub delete_id: Option<i64>,
    /// 开始日期
    pub date_from: Option<String>,
    /// 结束日期
    pub date_to: Option<String>,
    /// 产品 ID（筛选包含该产品的 BOM）
    #[serde(default, deserialize_with = "deserialize_null_i64")]
    pub product_id: Option<i64>,
    /// 产品编码（筛选 BOM 第一个节点的产品编码）
    pub product_code: Option<String>,
    /// BOM 分类 ID
    #[serde(default, deserialize_with = "deserialize_null_i64")]
    pub bom_category_id: Option<i64>,
    /// 返回 URL
    pub back_url: Option<String>,
    /// 页码
    pub page: Option<i64>,
    /// 每页数量
    pub page_size: Option<i64>,
    /// BOM 状态过滤（由 handler 从 proto 转换）
    #[serde(skip)]
    pub status: Option<BomStatus>,
    /// 调用者 ID（由 handler 注入，用于可见性过滤）
    #[serde(skip)]
    pub caller_id: Option<i64>,
}

impl Default for BomQuery {
    fn default() -> Self {
        Self {
            bom_name: None,
            create_by: None,
            delete_id: None,
            date_from: None,
            date_to: None,
            product_id: None,
            product_code: None,
            bom_category_id: None,
            back_url: None,
            page: Some(1),
            page_size: Some(12),
            status: None,
            caller_id: None,
        }
    }
}

/// 反序列化可能为 null 或字符串的 i64
pub fn deserialize_null_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;

    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => Ok(s.parse().ok()),
        serde_json::Value::Number(num) => Ok(num.as_i64()),
        _ => Ok(None),
    }
}

// ============================================================================
// 成本报告
// ============================================================================

/// BOM 人工成本报告（单独查看）
#[derive(Debug, Clone)]
pub struct BomLaborCostReport {
    pub bom_id: i64,
    pub bom_name: String,
    pub product_code: String,
    pub labor_costs: Vec<LaborCostItem>,
    pub warnings: Vec<String>,
}

/// BOM 成本报告
#[derive(Debug, Clone)]
pub struct BomCostReport {
    pub bom_id: i64,
    pub bom_name: String,
    pub product_code: String,
    pub material_costs: Vec<MaterialCostItem>,
    pub labor_costs: Vec<LaborCostItem>,
    pub warnings: Vec<String>,
}

/// 材料成本项
#[derive(Debug, Clone)]
pub struct MaterialCostItem {
    pub node_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: f64,
    pub unit_price: Option<String>,
}

/// 人工成本项
#[derive(Debug, Clone)]
pub struct LaborCostItem {
    pub id: i64,
    pub name: String,
    pub unit_price: String,
    pub quantity: String,
    pub sort_order: i32,
    pub remark: String,
}

// ============================================================================
// 创建/更新请求
// ============================================================================

/// 创建 BOM 请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBomRequest {
    /// BOM 名称
    pub bom_name: String,
    /// 创建者（用户 ID）
    pub created_by: Option<i64>,
    /// BOM 分类 ID
    #[serde(default, deserialize_with = "deserialize_null_i64")]
    pub bom_category_id: Option<i64>,
}

/// 更新 BOM 请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBomRequest {
    /// BOM 名称
    pub bom_name: String,
    /// BOM 详情
    pub bom_detail: Option<BomDetail>,
    /// BOM 分类 ID
    #[serde(default, deserialize_with = "deserialize_null_i64")]
    pub bom_category_id: Option<i64>,
}

/// 添加 BOM 节点请求
#[derive(Debug, Serialize, Deserialize)]
pub struct AddBomNodeRequest {
    /// BOM ID
    pub bom_id: i64,
    /// 节点数据
    pub node: BomNode,
}

/// 删除 BOM 节点请求
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteBomNodeRequest {
    /// BOM ID
    pub bom_id: i64,
    /// 节点 ID
    pub node_id: i64,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bom_node_default() {
        let node = BomNode::default();
        assert_eq!(node.id, 0);
        assert_eq!(node.product_id, 0);
        assert_eq!(node.quantity, 0.0);
        assert_eq!(node.parent_id, 0);
        assert_eq!(node.loss_rate, 0.0);
        assert_eq!(node.order, 0);
        assert!(node.unit.is_none());
        assert!(node.remark.is_none());
    }

    #[test]
    fn test_bom_node_serialization() {
        let node = BomNode {
            id: 1,
            product_id: 100,
            product_code: Some("P001".to_string()),
            quantity: 10.5,
            parent_id: 0,
            loss_rate: 0.05,
            order: 1,
            unit: Some("个".to_string()),
            remark: Some("测试备注".to_string()),
            position: Some("A1".to_string()),
            work_center: Some("WC01".to_string()),
            properties: Some("自定义属性".to_string()),
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains(r#""id":1"#));
        assert!(json.contains(r#""product_id":100"#));
        assert!(json.contains(r#""quantity":10.5"#));
        assert!(json.contains(r#""unit":"个""#));

        // 反序列化
        let deserialized: BomNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.product_id, 100);
        assert_eq!(deserialized.quantity, 10.5);
        assert_eq!(deserialized.unit, Some("个".to_string()));
    }

    #[test]
    fn test_bom_detail_serialization() {
        let detail = BomDetail {
            nodes: vec![BomNode {
                id: 1,
                product_id: 100,
                product_code: Some("P001".to_string()),
                quantity: 5.0,
                parent_id: 0,
                loss_rate: 0.0,
                order: 1,
                unit: Some("个".to_string()),
                remark: None,
                position: None,
                work_center: None,
                properties: None,
            }],
        };

        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains(r#""nodes""#));

        let deserialized: BomDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.nodes.len(), 1);
    }

    #[test]
    fn test_bom_query_default() {
        let query = BomQuery::default();
        assert!(query.bom_name.is_none());
        assert!(query.create_by.is_none());
        assert_eq!(query.page, Some(1));
        assert_eq!(query.page_size, Some(12));
    }

    #[test]
    fn test_deserialize_null_i64_with_null() {
        let json = r#"{"value": null}"#;
        #[derive(Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "deserialize_null_i64")]
            value: Option<i64>,
        }
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert!(result.value.is_none());
    }

    #[test]
    fn test_deserialize_null_i64_with_string() {
        let json = r#"{"value": "123"}"#;
        #[derive(Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "deserialize_null_i64")]
            value: Option<i64>,
        }
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(123));
    }

    #[test]
    fn test_deserialize_null_i64_with_number() {
        let json = r#"{"value": 456}"#;
        #[derive(Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "deserialize_null_i64")]
            value: Option<i64>,
        }
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(456));
    }

    #[test]
    fn test_create_bom_request() {
        let request = CreateBomRequest {
            bom_name: "测试BOM".to_string(),
            created_by: Some(42),
            bom_category_id: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""bom_name":"测试BOM""#));

        let deserialized: CreateBomRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bom_name, "测试BOM");
        assert_eq!(deserialized.created_by, Some(42));
        assert_eq!(deserialized.bom_category_id, None);
    }
}
