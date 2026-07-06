use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Entity ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrintTemplate {
    pub id: i64,
    pub name: String,
    pub document_type: String,
    pub description: Option<String>,
    pub html_content: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ── Request / Response ──

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePrintTemplateReq {
    pub name: String,
    pub document_type: String,
    #[serde(default)]
    pub description: Option<String>,
    pub html_content: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePrintTemplateReq {
    pub name: Option<String>,
    pub document_type: Option<String>,
    pub description: Option<String>,
    pub html_content: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrintTemplateQuery {
    pub document_type: Option<String>,
    pub keyword: Option<String>,
}

// ── Render context: what gets passed when rendering a template ──

/// 模板渲染上下文（minijinja / Jinja2 语法）。
/// Object 顶层 key 即模板变量名（支持中文，如 `{{ 客户全称 }}`，需 minijinja `unicode` feature）；
/// Array 用于明细行循环（`{% for item in 明细 %}{{ item.产品名称 }}{% endfor %}`）。
pub type RenderVars = serde_json::Value;
