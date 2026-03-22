//! BOM 人工工序数据模型

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// BOM 人工工序（通过产品编码关联 BOM）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomLaborProcess {
    pub id: i64,
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 创建人工工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessRequest {
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 更新人工工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessRequest {
    pub id: i64,
    pub product_code: String,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 查询人工工序列表请求（按产品编码查询）
#[derive(Debug, Clone, Deserialize)]
pub struct ListLaborProcessRequest {
    pub product_code: String,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// 人工工序成本信息
#[derive(Debug, Clone)]
pub struct LaborProcessCost {
    pub process: BomLaborProcess,
    pub subtotal: f64,
}

/// 导入结果
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub success_count: u64,
    pub fail_count: u64,
    pub errors: Vec<String>,
}

impl BomLaborProcess {
    /// 计算单道工序成本
    pub fn calculate_cost(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        (self.unit_price * self.quantity).to_f64().unwrap_or(0.0)
    }
}

/// 计算总人工成本
pub fn calculate_total_cost(processes: &[BomLaborProcess]) -> f64 {
    processes.iter().map(|p| p.calculate_cost()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn test_calculate_cost() {
        let process = BomLaborProcess {
            id: 1,
            product_code: "P001".to_string(),
            name: "切割".to_string(),
            unit_price: dec("50.00"),
            quantity: dec("100"),
            sort_order: 0,
            remark: None,
            created_at: Utc::now(),
            updated_at: None,
        };

        assert_eq!(process.calculate_cost(), 5000.0);
    }

    #[test]
    fn test_calculate_total_cost() {
        let processes = vec![
            BomLaborProcess {
                id: 1,
                product_code: "P001".to_string(),
                name: "切割".to_string(),
                unit_price: dec("50.00"),
                quantity: dec("100"),
                sort_order: 0,
                remark: None,
                created_at: Utc::now(),
                updated_at: None,
            },
            BomLaborProcess {
                id: 2,
                product_code: "P001".to_string(),
                name: "焊接".to_string(),
                unit_price: dec("80.00"),
                quantity: dec("50"),
                sort_order: 1,
                remark: None,
                created_at: Utc::now(),
                updated_at: None,
            },
        ];

        assert_eq!(calculate_total_cost(&processes), 9000.0); // 5000 + 4000
    }
}
