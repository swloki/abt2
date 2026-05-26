//! H3Yun REST API 客户端

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::warn;

use crate::h3yun::models::{H3YunFilter, H3YunRequest, H3YunResponse, SyncError, action};

const DEFAULT_ENDPOINT: &str = "https://www.h3yun.com/OpenApi/Invoke";
const DEFAULT_ENGINE_CODE: &str = "wkcmav3emlzu0l1smysmopu85";
const DEFAULT_ENGINE_SECRET: &str = "KzoufliRxIlLQkt9DBiXd64PlnXJjNuu+rR+RATVEt8RvB1yj+DuUg==";

#[derive(Clone, Default)]
pub struct H3YunClient {
    client: Client,
    endpoint: String,
    engine_code: String,
    engine_secret: String,
}

// Manual Debug to avoid leaking credentials in logs
impl std::fmt::Debug for H3YunClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H3YunClient")
            .field("endpoint", &self.endpoint)
            .field("engine_code", &"[REDACTED]")
            .finish()
    }
}

impl H3YunClient {
    /// 创建客户端，优先从环境变量读取凭证，未设置则使用默认值
    pub fn new() -> Self {
        let engine_code = std::env::var("H3YUN_ENGINE_CODE").unwrap_or_else(|_| {
            warn!("H3YUN_ENGINE_CODE not set, using default");
            DEFAULT_ENGINE_CODE.to_string()
        });
        let engine_secret = std::env::var("H3YUN_ENGINE_SECRET").unwrap_or_else(|_| {
            warn!("H3YUN_ENGINE_SECRET not set, using default");
            DEFAULT_ENGINE_SECRET.to_string()
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            endpoint: DEFAULT_ENDPOINT.to_string(),
            engine_code,
            engine_secret,
        }
    }

    /// 创建 BizObject，返回 ObjectId
    pub async fn create(&self, schema_code: &str, biz_object: &str) -> Result<String, SyncError> {
        let req = H3YunRequest {
            ActionName: action::CREATE.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: Some(biz_object.to_string()),
            BizObjectId: None,
            IsSubmit: Some(true),
        };

        let resp = self.invoke(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        let return_data = resp.ReturnData;
        return_data
            .and_then(|d| {
                d.get("BizObjectId")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .or_else(|| {
                        d.get("ObjectIds").cloned()
                            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                            .and_then(|ids| ids.into_iter().next())
                    })
            })
            .ok_or_else(|| SyncError::FatalError {
                reason: "CreateBizObject succeeded but no ObjectId in ReturnData".to_string(),
            })
    }

    pub async fn update(
        &self,
        schema_code: &str,
        object_id: &str,
        biz_object: &str,
    ) -> Result<(), SyncError> {
        // 和旧代码一致：BizObjectId 作为顶层字段，BizObject 是 JSON 字符串
        let req = H3YunRequest {
            ActionName: action::UPDATE.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: Some(biz_object.to_string()),
            BizObjectId: Some(object_id.to_string()),
            IsSubmit: Some(true),
        };

        let resp = self.invoke(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        Ok(())
    }

    /// 删除 BizObject
    pub async fn delete(&self, schema_code: &str, object_id: &str) -> Result<(), SyncError> {
        let req = H3YunRequest {
            ActionName: action::REMOVE.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: None,
            BizObjectId: Some(object_id.to_string()),
            IsSubmit: None,
        };

        let resp = self.invoke(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        Ok(())
    }

    /// 查询 BizObject 列表（对账用）
    pub async fn query_list(&self, schema_code: &str) -> Result<Vec<Value>, SyncError> {
        let filter = serde_json::json!({
            "FromRowNum": 0,
            "RequireCount": false,
            "ReturnItems": [],
            "SortByCollection": [],
            "ToRowNum": 10000
        });
        let req = H3YunFilter {
            ActionName: action::LOAD.to_string(),
            SchemaCode: schema_code.to_string(),
            Filter: filter.to_string(),
        };

        let resp = self.invoke_filter(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        let items = resp
            .ReturnData
            .and_then(|d| {
                if let Some(s) = d.as_str() {
                    serde_json::from_str::<Vec<Value>>(s).ok()
                } else if d.is_array() {
                    serde_json::from_value::<Vec<Value>>(d).ok()
                } else {
                    d.as_object()
                        .and_then(|o| o.get("BizObjectArray"))
                        .and_then(|v| serde_json::from_value::<Vec<Value>>(v.clone()).ok())
                }
            })
            .unwrap_or_default();

        Ok(items)
    }

    /// 按字段查询 ObjectId，存在返回 Some(object_id)
    pub async fn find_by_field(
        &self,
        schema_code: &str,
        field_name: &str,
        field_value: &str,
    ) -> Result<Option<String>, SyncError> {
        // 和旧代码 ListFilter 结构完全一致，包含所有字段
        let filter = serde_json::json!({
            "FromRowNum": 0,
            "RequireCount": true,
            "ReturnItems": ["ObjectId"],
            "SortByCollection": [],
            "ToRowNum": 12,
            "Matcher": {
                "Type": "Item",
                "Matchers": null,
                "Name": field_name,
                "Operator": 2,
                "Value": field_value
            }
        });

        // LoadBizObjects 用 "Filter" 字段，不是 "BizObject"
        let req = H3YunFilter {
            ActionName: action::LOAD.to_string(),
            SchemaCode: schema_code.to_string(),
            Filter: filter.to_string(),
        };

        let resp = self.invoke_filter(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        let return_data_debug = format!("{:?}", resp.ReturnData);
        let object_id = resp.ReturnData.and_then(|d| {
            let data = if let Some(s) = d.as_str() {
                serde_json::from_str::<Value>(s).ok().unwrap_or(d)
            } else {
                d
            };
            data.get("BizObjectArray")
                .and_then(|arr| arr.as_array())
                .and_then(|items| items.first())
                .and_then(|item| item.get("ObjectId"))
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        });

        if object_id.is_none() {
            warn!(
                schema_code,
                field_name,
                field_value,
                return_data = %return_data_debug,
                "find_by_field: query returned no ObjectId"
            );
        }

        Ok(object_id)
    }

    /// 按多字段 AND 查询 ObjectId（和旧代码三字段联合匹配一致）
    pub async fn find_by_fields(
        &self,
        schema_code: &str,
        fields: &[(&str, &str)],
    ) -> Result<Option<String>, SyncError> {
        let matchers: Vec<Value> = fields
            .iter()
            .map(|(name, value)| {
                serde_json::json!({
                    "Type": "Item",
                    "Matchers": null,
                    "Name": name,
                    "Operator": 2,
                    "Value": value
                })
            })
            .collect();

        let filter = serde_json::json!({
            "FromRowNum": 0,
            "RequireCount": true,
            "ReturnItems": ["ObjectId"],
            "SortByCollection": [],
            "ToRowNum": 12,
            "Matcher": {
                "Type": "And",
                "Matchers": matchers,
                "Name": null,
                "Operator": null,
                "Value": null
            }
        });

        let req = H3YunFilter {
            ActionName: action::LOAD.to_string(),
            SchemaCode: schema_code.to_string(),
            Filter: filter.to_string(),
        };

        let resp = self.invoke_filter(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        let object_id = resp.ReturnData.and_then(|d| {
            let data = if let Some(s) = d.as_str() {
                serde_json::from_str::<Value>(s).ok().unwrap_or(d)
            } else {
                d
            };
            data.get("BizObjectArray")
                .and_then(|arr| arr.as_array())
                .and_then(|items| items.first())
                .and_then(|item| item.get("ObjectId"))
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        });

        Ok(object_id)
    }

    /// 发送 HTTP 请求（Filter 格式，用于 LoadBizObjects）
    async fn invoke_filter(&self, req: &H3YunFilter) -> Result<H3YunResponse, SyncError> {
        self.do_post(req).await
    }

    /// 发送 HTTP 请求（BizObject 格式，用于 Create/Update/Remove）
    async fn invoke(&self, req: &H3YunRequest) -> Result<H3YunResponse, SyncError> {
        self.do_post(req).await
    }

    async fn do_post<T: serde::Serialize>(&self, req: &T) -> Result<H3YunResponse, SyncError> {
        let resp = self
            .client
            .post(&self.endpoint)
            .header("EngineCode", &self.engine_code)
            .header("EngineSecret", &self.engine_secret)
            .json(req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    SyncError::Transient {
                        backoff_hint: Duration::from_secs(5),
                    }
                } else {
                    SyncError::FatalError {
                        reason: format!("HTTP request failed: {e}"),
                    }
                }
            })?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(SyncError::Transient {
                backoff_hint: Duration::from_secs(30),
            });
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(SyncError::FatalError {
                reason: format!("Authentication failed: HTTP {}", status),
            });
        }

        let body_text = resp.text().await.map_err(|e| SyncError::FatalError {
            reason: format!("Failed to read response body: {e}"),
        })?;

        serde_json::from_str::<H3YunResponse>(&body_text).map_err(|e| {
            let preview = if body_text.len() > 500 {
                format!("{}... (truncated, total {} bytes)", &body_text[..500], body_text.len())
            } else {
                body_text.clone()
            };
            warn!(status = %status, error = %e, body = %preview, "Failed to parse H3Yun response as JSON");

            if status.is_server_error() {
                SyncError::Transient {
                    backoff_hint: Duration::from_secs(30),
                }
            } else {
                SyncError::FatalError {
                    reason: format!("Invalid JSON response from H3Yun (HTTP {}): {e}. Body: {}", status, preview),
                }
            }
        })
    }
}

/// 根据 H3Yun 响应分类错误
fn classify_error(message: &str) -> SyncError {
    let msg_lower = message.to_lowercase();
    if msg_lower.contains("timeout")
        || msg_lower.contains("rate")
        || msg_lower.contains("too many")
        || msg_lower.contains("server")
        || msg_lower.contains("unavailable")
        || msg_lower.contains("internal")
        || msg_lower.contains("connection")
    {
        SyncError::ValidationError {
            record_id: String::new(),
            fields: vec![format!("H3Yun 暂时性错误: {message}")],
        }
    } else if msg_lower.is_empty()
        || msg_lower.contains("objectid")
        || msg_lower.contains("duplicate")
        || msg_lower.contains("已存在")
    {
        SyncError::ValidationError {
            record_id: String::new(),
            fields: vec![if message.is_empty() { "未知错误".to_string() } else { message.to_string() }],
        }
    } else if msg_lower.contains("auth")
        || msg_lower.contains("credential")
        || msg_lower.contains("schema")
    {
        SyncError::FatalError {
            reason: message.to_string(),
        }
    } else if msg_lower.contains("required")
        || msg_lower.contains("invalid format")
        || msg_lower.contains("duplicate")
    {
        SyncError::ValidationError {
            record_id: String::new(),
            fields: vec![message.to_string()],
        }
    } else {
        // 未知错误直接暴露原始信息
        SyncError::ValidationError {
            record_id: String::new(),
            fields: vec![if message.is_empty() { "未知错误".to_string() } else { message.to_string() }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn is_transient(err: SyncError) -> bool {
        matches!(err, SyncError::Transient { .. })
    }
    fn is_fatal(err: SyncError) -> bool {
        matches!(err, SyncError::FatalError { .. })
    }
    fn is_validation(err: SyncError) -> bool {
        matches!(err, SyncError::ValidationError { .. })
    }

    #[test]
    fn classify_transient_timeout() {
        assert!(is_transient(classify_error("Request timeout")));
    }

    #[test]
    fn classify_transient_rate() {
        assert!(is_transient(classify_error("Rate limit exceeded")));
    }

    #[test]
    fn classify_transient_server() {
        assert!(is_transient(classify_error("Internal Server Error")));
    }

    #[test]
    fn classify_transient_unavailable() {
        assert!(is_transient(classify_error("Service Unavailable")));
    }

    #[test]
    fn classify_fatal_auth() {
        assert!(is_fatal(classify_error("Authentication failed")));
    }

    #[test]
    fn classify_fatal_credential() {
        assert!(is_fatal(classify_error("Invalid credential")));
    }

    #[test]
    fn classify_validation_required() {
        assert!(is_validation(classify_error("Required field missing")));
    }

    #[test]
    fn classify_validation_duplicate() {
        assert!(is_validation(classify_error("Duplicate key")));
    }

    #[test]
    fn classify_unknown_defaults_to_transient() {
        assert!(is_transient(classify_error("Some unknown error")));
    }

    #[test]
    fn classify_empty_defaults_to_transient() {
        assert!(is_transient(classify_error("")));
    }
}
