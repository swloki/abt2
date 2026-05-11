//! H3Yun REST API 客户端

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::warn;

use super::models::{action, H3YunRequest, H3YunResponse, SyncError};

const DEFAULT_ENDPOINT: &str = "https://www.h3yun.com/OpenApi/Invoke";
const DEFAULT_ENGINE_CODE: &str = "wkcmav3emlzu0l1smysmopu85";
const DEFAULT_ENGINE_SECRET: &str = "PO+ZqVdtElYtTteED8z0wPUs5QBP/3WoXzGj4PEYYyKl0riiEhB8Rw==";

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
            BizObject: biz_object.to_string(),
            IsSubmit: Some(true),
        };

        let resp = self.invoke(&req).await?;
        if !resp.Successful {
            return Err(classify_error(&resp.ErrorMessage));
        }

        resp.ReturnData
            .and_then(|d| d.get("ObjectIds").cloned())
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .and_then(|ids| ids.into_iter().next())
            .ok_or_else(|| SyncError::FatalError {
                reason: "CreateBizObject succeeded but no ObjectId returned".to_string(),
            })
    }

    pub async fn update(
        &self,
        schema_code: &str,
        object_id: &str,
        biz_object: &str,
    ) -> Result<(), SyncError> {
        // H3Yun UpdateBizObject requires ObjectId in BizObject
        let mut payload: serde_json::Value = serde_json::from_str(biz_object)
            .map_err(|e| SyncError::FatalError {
                reason: format!("Failed to parse biz_object as JSON for update: {e}"),
            })?;
        payload["ObjectId"] = serde_json::Value::String(object_id.to_string());

        let req = H3YunRequest {
            ActionName: action::UPDATE.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: payload.to_string(),
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
        let biz_object = serde_json::json!({ "ObjectId": object_id }).to_string();
        let req = H3YunRequest {
            ActionName: action::REMOVE.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: biz_object,
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
        let biz_object = serde_json::json!({ "Filter": "" }).to_string();
        let req = H3YunRequest {
            ActionName: action::LOAD.to_string(),
            SchemaCode: schema_code.to_string(),
            BizObject: biz_object,
            IsSubmit: None,
        };

        let resp = self.invoke(&req).await?;
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
                        .and_then(|o| o.get("ObjectDatas"))
                        .and_then(|v| serde_json::from_value::<Vec<Value>>(v.clone()).ok())
                }
            })
            .unwrap_or_default();

        Ok(items)
    }

    /// 发送 HTTP 请求
    async fn invoke(&self, req: &H3YunRequest) -> Result<H3YunResponse, SyncError> {
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

        resp.json::<H3YunResponse>()
            .await
            .map_err(|e| {
                warn!(status = %status, error = %e, "Failed to parse H3Yun response as JSON");
                SyncError::FatalError {
                    reason: format!("Invalid JSON response from H3Yun: {e}"),
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
        SyncError::Transient {
            backoff_hint: Duration::from_secs(10),
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
        // Default to Transient for unrecognized errors — safer to retry than to skip
        SyncError::Transient {
            backoff_hint: Duration::from_secs(10),
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
