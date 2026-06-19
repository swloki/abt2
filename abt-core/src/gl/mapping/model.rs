//! 科目映射模型

/// 科目映射解析结果
#[derive(Debug, Clone)]
pub struct AccountMapping {
    pub id: i64,
    pub mapping_key: String,
    pub account_id: i64,
    pub product_id: Option<i64>,
}
