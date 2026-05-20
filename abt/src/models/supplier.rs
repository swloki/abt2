//! 供应商数据模型
//!
//! 包含供应商实体及其联系人、银行账户等关联结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 供应商实体
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct Supplier {
    pub supplier_id: i64,
    pub supplier_code: String,
    pub supplier_name: String,
    pub short_name: Option<String>,
    pub classification: String,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 供应商联系人
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierContact {
    pub contact_id: i64,
    pub supplier_id: i64,
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

/// 供应商银行账户
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierBankAccount {
    pub bank_account_id: i64,
    pub supplier_id: i64,
    pub bank_name: String,
    pub account_name: String,
    pub account_no: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// 供应商查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SupplierQuery {
    /// 关键词（模糊匹配 supplier_name 和 supplier_code）
    pub keyword: Option<String>,
    /// 分类过滤
    pub classification: Option<String>,
    /// 状态过滤
    pub status: Option<i16>,
    /// 页码
    pub page: Option<i64>,
    /// 每页数量
    pub page_size: Option<i64>,
}

/// 供应商详情（含联系人和银行账户）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SupplierDetail {
    #[serde(flatten)]
    pub supplier: Supplier,
    pub contacts: Vec<SupplierContact>,
    pub bank_accounts: Vec<SupplierBankAccount>,
}
