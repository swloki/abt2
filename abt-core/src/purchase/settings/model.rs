use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 采购参数配置实体（单行表，id=1）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseSettings {
    pub id: i64,
    pub over_delivery_allowance_pct: Decimal,
    pub over_shortage_allowance_pct: Decimal,
    pub maintain_same_rate: bool,
    pub po_required_for_receipt: bool,
    pub receipt_required_for_invoice: bool,
    pub default_currency_code: String,
    pub default_tax_rate_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 配置行读取失败时的回退默认值（零容差、禁用全部业务规则开关、CNY）。
impl Default for PurchaseSettings {
    fn default() -> Self {
        Self {
            id: 1,
            over_delivery_allowance_pct: Decimal::ZERO,
            over_shortage_allowance_pct: Decimal::ZERO,
            maintain_same_rate: false,
            po_required_for_receipt: false,
            receipt_required_for_invoice: false,
            default_currency_code: String::from("CNY"),
            default_tax_rate_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

/// 更新采购参数请求
#[derive(Debug, Clone, Default)]
pub struct UpdatePurchaseSettingsRequest {
    pub over_delivery_allowance_pct: Option<Decimal>,
    pub over_shortage_allowance_pct: Option<Decimal>,
    pub maintain_same_rate: Option<bool>,
    pub po_required_for_receipt: Option<bool>,
    pub receipt_required_for_invoice: Option<bool>,
    pub default_currency_code: Option<String>,
    pub default_tax_rate_id: Option<Option<i64>>,
}
