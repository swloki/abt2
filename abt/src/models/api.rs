//! API 相关数据模型
//!
//! 包含 H3Yun API 客户端相关的数据结构。

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// H3Yun API 请求/响应结构
// ============================================================================

/// 列表查询过滤器
#[derive(Serialize, Debug)]
pub struct ListFilter<'a> {
    #[serde(rename = "FromRowNum")]
    pub from_row_num: u64,
    #[serde(rename = "RequireCount")]
    pub require_count: bool,
    #[serde(rename = "ReturnItems")]
    pub return_items: Vec<&'a str>,
    #[serde(rename = "SortByCollection")]
    pub sort_by_collection: Vec<&'a str>,
    #[serde(rename = "ToRowNum")]
    pub to_row_num: u64,
    #[serde(rename = "Matcher")]
    pub matcher: Matcher,
}

impl ListFilter<'_> {
    pub fn new() -> Self {
        ListFilter {
            from_row_num: 0,
            require_count: true,
            return_items: vec![],
            sort_by_collection: vec![],
            to_row_num: 12,
            matcher: Matcher {
                matcher_type: MatcherType::And,
                matchers: Some(vec![]),
                name: None,
                operator: None,
                value: None,
            },
        }
    }
}

impl Default for ListFilter<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// 匹配器类型
#[derive(Serialize, Deserialize, Debug)]
pub enum MatcherType {
    And,
    Or,
    Item,
}

/// 比较操作符
#[derive(Serialize, Deserialize, Debug)]
pub enum Operator {
    GreaterThan = 0,
    GreaterThanOrEqual = 1,
    Equal = 2,
    LessThanOrEqual = 3,
    LessThan = 4,
    NotEqual = 5,
    InRange = 6,
    NotInRange = 7,
}

/// 匹配器
#[derive(Serialize, Deserialize, Debug)]
pub struct Matcher {
    #[serde(rename = "Type")]
    pub matcher_type: MatcherType,
    #[serde(rename = "Matchers")]
    pub matchers: Option<Vec<Matcher>>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "Operator")]
    pub operator: Option<Operator>,
    #[serde(rename = "Value")]
    pub value: Option<String>,
}

/// H3Yun API 响应
#[derive(Serialize, Deserialize)]
pub struct DataResponse {
    #[serde(rename = "Successful")]
    pub successful: bool,
    #[serde(rename = "ErrorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "Logined")]
    pub logined: bool,
    #[serde(rename = "ReturnData")]
    pub return_data: Value,
    #[serde(rename = "DataType")]
    pub data_type: Option<u64>,
}

/// 返回的数组数据
#[derive(Serialize, Deserialize)]
pub struct ReturnArrayData<T> {
    #[serde(rename = "BizObjectArray")]
    pub biz_object_array: Option<Vec<T>>,
    #[serde(rename = "TotalCount")]
    pub total_count: Option<u64>,
}

/// 返回的单条数据
#[derive(Serialize, Deserialize, Debug)]
pub struct ReturnData<T> {
    #[serde(rename = "BizObject")]
    pub biz_object: Option<T>,
}

// ============================================================================
// API 配置
// ============================================================================

/// H3Yun API 配置
#[derive(Debug, Clone)]
pub struct AbtApiConfig {
    pub engine_code: String,
    pub engine_secret: String,
    pub api_path: String,
}

impl Default for AbtApiConfig {
    fn default() -> Self {
        Self {
            engine_code: String::new(),
            engine_secret: String::new(),
            api_path: "https://www.h3yun.com/OpenApi/Invoke".to_string(),
        }
    }
}
