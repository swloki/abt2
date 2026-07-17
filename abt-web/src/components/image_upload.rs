//! 通用图片上传控件（即时上传拿路径，多组件复用）。
//!
//! 模式：选图 → 即时 POST 上传到 `static/uploads/{sub_dir}/{uuid}.{ext}`
//! → 返回缩略图片段（含 data-path/name/type/size）→ JS 把元信息累积到
//! hidden `attachments_json`。新建表单提交时携带该 JSON，后端据此关联到新单据。
//!
//! 用法（新建表单内）：
//!   `(image_upload("attachments_json", "quotation"))`
//! 删除按钮 hyperscript `_="on click call imageRemove(me)"`，
//! 预览容器 afterSettle `_="on htmx:afterSettle call imageUploadSync(closest .image-upload)"`。

use axum::extract::{Multipart, Query};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::attachment::{Attachment, AttachmentService};
use abt_core::shared::types::DomainError;

use crate::errors::Result;
use crate::state::AppState;
use crate::utils::RequestContext;

// ── 路径 ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/components/upload-image")]
pub struct UploadImagePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/components/delete-image")]
pub struct DeleteImagePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/components/attachments/{owner_type}/{owner_id}")]
pub struct AttachmentsListPath {
    pub owner_type: String,
    pub owner_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/components/attachments/{id}/delete")]
pub struct AttachmentDeletePath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(UploadImagePath::PATH, post(upload_image))
        .route(DeleteImagePath::PATH, post(delete_image))
        .route(AttachmentsListPath::PATH, get(list_attachments))
        .route(AttachmentDeletePath::PATH, post(delete_attachment))
}

// ── 端点 ──

#[derive(Deserialize)]
pub struct UploadParams {
    pub sub_dir: String,
}

/// POST 上传图片（支持多张）：存盘 → 返回缩略图片段（拼接多张）。
/// 登录即可（auth middleware 保证）；具体业务权限在创建单据时校验。
pub async fn upload_image(
    Query(params): Query<UploadParams>,
    mut multipart: Multipart,
) -> Result<Html<String>> {
    // sub_dir 白名单：字母/数字/下划线，防路径穿越
    if params.sub_dir.is_empty()
        || !params
            .sub_dir
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DomainError::validation("无效的上传目录").into());
    }

    const MAX_SIZE: usize = 10 * 1024 * 1024;
    let mut out = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let fname = field.file_name().unwrap_or("image.png").to_string();
        let ctype = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .to_vec();
        // 静默跳过非图片 / 超大
        if !ctype.starts_with("image/") || bytes.len() > MAX_SIZE || bytes.is_empty() {
            continue;
        }
        let ext = std::path::Path::new(&fname)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "png".to_string());
        let stored_path = format!("{}/{}.{}", params.sub_dir, uuid::Uuid::new_v4(), ext);
        let full_path = format!("static/uploads/{}", stored_path);
        if let Some(parent) = std::path::Path::new(&full_path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        tokio::fs::write(&full_path, &bytes)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        out.push_str(&render_image_item(&stored_path, &fname, &ctype, bytes.len() as i64).into_string());
    }
    Ok(Html(out))
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub path: String,
}

/// POST 删除已上传图片（按 stored_path 删文件）；记录未建时直接删文件。
pub async fn delete_image(Query(params): Query<DeleteParams>) -> Result<Html<String>> {
    // 防路径穿越
    if params.path.is_empty()
        || params.path.contains("..")
        || params.path.contains('\\')
        || params.path.starts_with('/')
    {
        return Err(DomainError::validation("无效的文件路径").into());
    }
    let full_path = format!("static/uploads/{}", params.path);
    // 文件可能已删（重复请求），失败静默
    let _ = tokio::fs::remove_file(&full_path).await;
    Ok(Html(String::new()))
}

// ── 组件 ──

/// 单张缩略图片段（上传端点返回；data-* 供 JS 同步到 attachments_json）。
fn render_image_item(stored_path: &str, file_name: &str, content_type: &str, size: i64) -> Markup {
    html! {
        div class="iu-item relative group"
            data-path=(stored_path) data-name=(file_name) data-type=(content_type)
            data-size=(size) {
            img src=(format!("/uploads/{}", stored_path)) alt=(file_name)
                class="w-24 h-24 object-cover rounded-sm border border-border-soft bg-surface";
            button type="button" title="移除"
                class="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-danger text-white text-xs leading-none grid place-items-center border-none cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity"
                _="on click call imageRemove(me)" { "×" }
        }
    }
}

/// 通用图片上传控件（嵌新建表单）。
/// - `field_name`：hidden input 的 name（携带路径 JSON，提交到后端）
/// - `sub_dir`：文件子目录（如 "quotation"，区分不同单据）
pub fn image_upload(field_name: &str, sub_dir: &str) -> Markup {
    html! {
        div class="image-upload" {
            form hx-post=(format!("{}?sub_dir={}", UploadImagePath::PATH, sub_dir))
                 hx-encoding="multipart/form-data"
                 hx-trigger="change"
                 hx-target=".image-upload-preview"
                 hx-swap="beforeend"
                 hx-disabled-elt="this" {
                label class="flex flex-col items-center justify-center gap-1.5 py-6 border border-dashed border-border rounded-sm cursor-pointer hover:border-accent hover:bg-accent-bg transition-colors duration-150" {
                    input type="file" name="file" accept="image/*" multiple class="hidden";
                    (crate::components::icon::upload_icon("w-6 h-6 text-muted"))
                    span class="text-xs text-fg-2" { "点击添加图片（可多选）" }
                    span class="text-[11px] text-muted" { "png / jpg / gif / webp，单张 ≤ 10MB" }
                }
            }
            div class="image-upload-preview flex flex-wrap gap-2 mt-2"
                _="on htmx:afterSettle call imageUploadSync(closest .image-upload)" {}
            input type="hidden" name=(field_name) class="image-upload-json" value="[]";
        }
    }
}

// ── 详情查看：通用附件列表/删除端点 + 组件 ──

/// GET 列出某单据的全部附件（缩略图网格片段）。
pub async fn list_attachments(
    path: AttachmentsListPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let items = state
        .attachment_service()
        .list(&service_ctx, &mut conn, &path.owner_type, path.owner_id)
        .await?;
    Ok(Html(render_attachment_list(&items).into_string()))
}

/// POST 删除附件（事务删记录 → commit → 删文件）。
pub async fn delete_attachment(
    path: AttachmentDeletePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let att = state
        .attachment_service()
        .delete(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = tokio::fs::remove_file(format!("static/uploads/{}", att.stored_path)).await;
    Ok(Html(String::new()))
}

/// 详情 drawer 附件区（只读查看 + 删除；上传在新建表单完成）。
/// `owner_type` 如 "quotation" / "sales_order"，与上传控件的 sub_dir 对齐。
pub fn attachment_section(owner_type: &str, owner_id: i64) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-md p-4" {
            div class="text-sm font-semibold text-fg mb-3" { "附件" }
            div class="att-list"
                hx-get=(AttachmentsListPath { owner_type: owner_type.to_string(), owner_id }.to_string())
                hx-trigger="load" {
                p class="text-xs text-muted py-3 text-center" { "加载中…" }
            }
        }
    }
}

/// 附件缩略图网格（list 端点返回；删除按钮 hx-swap=delete 移除单张）。
fn render_attachment_list(items: &[Attachment]) -> Markup {
    html! {
        @if items.is_empty() {
            p class="text-xs text-muted py-3 text-center" { "暂无附件" }
        } @else {
            div class="flex flex-wrap gap-2" {
                @for a in items {
                    div class="att-item relative group" {
                        a href=(format!("/uploads/{}", a.stored_path))
                            target="_blank" rel="noopener"
                            title=(format!("{} · {}", a.file_name, fmt_size(a.file_size))) {
                            img src=(format!("/uploads/{}", a.stored_path)) alt=(a.file_name.as_str())
                                class="w-24 h-24 object-cover rounded-sm border border-border-soft bg-surface hover:opacity-80 transition-opacity";
                        }
                        button type="button" title="删除"
                            class="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-danger text-white text-xs leading-none grid place-items-center border-none cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity"
                            hx-post=(AttachmentDeletePath { id: a.id }.to_string())
                            hx-target="closest .att-item" hx-swap="delete"
                            hx-confirm="确认删除该附件？" { "×" }
                    }
                }
            }
        }
    }
}

fn fmt_size(bytes: i64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb < 1.0 {
        format!("{bytes} B")
    } else if kb < 1024.0 {
        format!("{kb:.0} KB")
    } else {
        format!("{:.1} MB", kb / 1024.0)
    }
}
