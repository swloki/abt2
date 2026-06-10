# 上传导出功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在产品、BOM、工艺、仓库库位四个模块的列表页上，补齐导入/导出前端交互入口和 Web handler 层。

**Architecture:** 通用 Maud 组件（import_modal + export_button）+ 集中式 excel.rs 路由（TypedPath + Handler 分发）。导入用 `tokio::spawn` + 内存 ProgressStore 实现异步进度跟踪；导出同步执行直接返回下载链接。

**Tech Stack:** Axum + Maud + HTMX + Surreal.js + rust_xlsxwriter（模板生成）+ DashMap（内存进度存储）

---

## File Structure

### 新增文件

| 文件 | 职责 |
|------|------|
| `abt-web/src/components/import_modal.rs` | 通用导入 Modal 组件（文件选择 + 进度条 + 结果展示） |
| `abt-web/src/components/export_button.rs` | 通用导出按钮 + 下拉菜单组件 |
| `abt-web/src/routes/excel.rs` | 所有导入导出的 TypedPath + Handler + 模板生成 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `abt-web/src/components/mod.rs` | 新增 `pub mod import_modal;` `pub mod export_button;` |
| `abt-web/src/state.rs` | AppState 新增 `import_progress` 和 `export_files` 字段 |
| `abt-web/src/routes/mod.rs` | 新增 `pub mod excel;` 和 `.merge(excel::router())` |
| `abt-web/src/pages/product_list.rs` | page-actions 添加导入/导出按钮 + 底部 Modal |
| `abt-web/src/pages/bom_list.rs` | page-actions 添加导出下拉菜单 |
| `abt-web/src/pages/bom_detail.rs` | page-actions 添加导出 BOM 按钮 |
| `abt-web/src/pages/labor_process_dict_list.rs` | page-actions 添加导出按钮 |
| `abt-web/src/pages/routing_list.rs` | page-actions 添加导入/导出按钮 + 底部 Modal |
| `abt-web/src/pages/wms_warehouse_list.rs` | page-actions 添加导入/导出按钮 + 底部 Modal |
| `uno.config.ts` 或 `static/base.css` | 新增导入导出相关 CSS 类 |

---

## Task 1: CSS 样式

**Files:**
- Modify: `static/base.css`（追加导入导出相关样式）

- [ ] **Step 1: 在 `static/base.css` 末尾追加导入导出样式类**

```css
/* ── Import Modal ── */
.import-file-zone {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.import-file-zone input[type="file"] {
  padding: var(--space-2);
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  cursor: pointer;
}
.import-file-zone input[type="file"]:hover {
  border-color: var(--primary);
  background: var(--primary-50);
}
.import-cols {
  font-size: var(--text-sm);
  color: var(--muted);
  background: var(--slate-50);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius);
}
.import-actions {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}
.import-progress-bar {
  height: 8px;
  background: var(--slate-100);
  border-radius: var(--radius);
  overflow: hidden;
}
.import-progress-fill {
  height: 100%;
  background: var(--primary);
  border-radius: var(--radius);
  transition: width 0.3s ease;
}
.import-result-stats {
  display: flex;
  gap: var(--space-4);
  margin-bottom: var(--space-3);
}
.import-stat {
  display: flex;
  flex-direction: column;
  align-items: center;
}
.import-stat-value {
  font-size: 1.5rem;
  font-weight: 700;
}
.import-stat-value.success { color: var(--green-600); }
.import-stat-value.failed { color: var(--red-600); }
.import-stat-label {
  font-size: var(--text-xs);
  color: var(--muted);
}
.import-errors {
  max-height: 200px;
  overflow-y: auto;
  background: var(--red-50);
  border: 1px solid var(--red-200);
  border-radius: var(--radius);
  padding: var(--space-2) var(--space-3);
}
.import-errors ul {
  list-style: disc;
  padding-left: var(--space-4);
  font-size: var(--text-sm);
  color: var(--red-700);
}

/* ── Export Dropdown ── */
.export-dropdown {
  position: relative;
  display: inline-block;
}
.export-dropdown-menu {
  display: none;
  position: absolute;
  right: 0;
  top: 100%;
  margin-top: 4px;
  background: white;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow-lg);
  z-index: 50;
  min-width: 180px;
}
.export-dropdown-menu.is-open {
  display: block;
}
.export-dropdown-menu button {
  display: flex;
  width: 100%;
  padding: var(--space-2) var(--space-3);
  font-size: var(--text-sm);
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
}
.export-dropdown-menu button:hover {
  background: var(--slate-50);
}

/* ── Export Result ── */
.export-result {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--green-50);
  border: 1px solid var(--green-200);
  border-radius: var(--radius);
  margin-top: var(--space-2);
  font-size: var(--text-sm);
}
.export-result a {
  text-decoration: none;
}
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: 添加导入导出组件 CSS 样式"
```

---

## Task 2: AppState 扩展 — ProgressStore

**Files:**
- Modify: `abt-web/src/state.rs`

先了解现有 `AppState` 的完整结构和 `new()` 方法，确认插入点。

- [ ] **Step 1: 在 `state.rs` 顶部新增 import**

在现有 `use` 块中追加：

```rust
use abt_core::shared::excel::{ImportProgress, ImportResult};
use chrono::{DateTime, Utc};
```

> 注意：`chrono` 已在 `abt-core/Cargo.toml` 的 workspace 中。检查 `abt-web/Cargo.toml` 是否已有 `chrono` — 是的，已有 `chrono = { workspace = true }`。

- [ ] **Step 2: 在 `state.rs` 中定义 ProgressStore 类型**

在 `AppState` 结构体定义之前添加：

```rust
use dashmap::DashMap;
use std::sync::atomic::AtomicI64;

/// 导入任务状态（内存存储）
#[derive(Debug)]
pub struct ImportTaskState {
    pub status: TaskStatus,
    pub current: usize,
    pub total: usize,
    pub result: Option<ImportResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

/// 导出文件信息（内存存储）
#[derive(Debug)]
pub struct ExportFileInfo {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 3: 在 `AppState` 结构体中新增两个字段**

```rust
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub session_store: FileSessionStorage,
    pub permission_cache: Arc<RolePermissionCache>,
    // 新增 ↓
    pub import_progress: Arc<DashMap<i64, ImportTaskState>>,
    pub export_files: Arc<DashMap<i64, ExportFileInfo>>,
    next_task_id: Arc<AtomicI64>,
}
```

- [ ] **Step 4: 在 `AppState::new()` 方法中初始化新字段**

在 `new()` 方法的返回值中追加字段：

```rust
import_progress: Arc::new(DashMap::new()),
export_files: Arc::new(DashMap::new()),
next_task_id: Arc::new(AtomicI64::new(1)),
```

- [ ] **Step 5: 添加辅助方法到 `AppState`**

在 `AppState` impl 块中追加：

```rust
/// 生成下一个 task_id
pub fn next_task_id(&self) -> i64 {
    self.next_task_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// 存储导出文件，返回 task_id
pub fn store_export_file(&self, bytes: Vec<u8>, filename: &str) -> i64 {
    let id = self.next_task_id();
    self.export_files.insert(id, ExportFileInfo {
        filename: filename.to_string(),
        bytes,
        created_at: Utc::now(),
    });
    id
}

/// 获取导出文件
pub fn get_export_file(&self, task_id: i64) -> Option<ExportFileInfo> {
    self.export_files.get(&task_id).map(|r| ExportFileInfo {
        filename: r.filename.clone(),
        bytes: r.bytes.clone(),
        created_at: r.created_at,
    })
}
```

- [ ] **Step 6: 在 `abt-web/Cargo.toml` 中添加 `dashmap` 依赖**

```toml
dashmap = "6"
```

- [ ] **Step 7: 运行 `cargo check` 验证编译通过**

```bash
cd abt-web && cargo check 2>&1 | tail -5
```

Expected: 编译通过，无错误

- [ ] **Step 8: Commit**

```bash
git add abt-web/src/state.rs abt-web/Cargo.toml
git commit -m "feat: AppState 增加 import_progress/export_files 内存存储"
```

---

## Task 3: 通用前端组件 — export_button.rs

**Files:**
- Create: `abt-web/src/components/export_button.rs`
- Modify: `abt-web/src/components/mod.rs`

- [ ] **Step 1: 创建 `export_button.rs`**

```rust
use maud::{html, Markup};

/// 导出项配置
pub struct ExportItem {
    pub label: &'static str,
    pub export_type: &'static str,
}

/// 单个导出按钮
pub fn export_button(label: &str, export_type: &str) -> Markup {
    let path = format!("/excel/export/{}", export_type);
    html! {
        button type="button" class="btn btn-default"
            hx-post=(path)
            hx-target="#export-result"
            hx-swap="innerHTML"
            hx-indicator="#export-result" {
            (crate::components::icon::download_icon("w-4 h-4"))
            " " (label)
        }
    }
}

/// 导出下拉菜单（多种导出类型）
pub fn export_dropdown(items: &[ExportItem]) -> Markup {
    let menu_buttons: Vec<Markup> = items.iter().map(|item| {
        let path = format!("/excel/export/{}", item.export_type);
        html! {
            button type="button"
                hx-post=(path)
                hx-target="#export-result"
                hx-swap="innerHTML"
                hx-indicator="#export-result"
                onclick="hsRemoveClosest(this,'.export-dropdown-menu','is-open')" {
                (item.label)
            }
        }
    }).collect();

    html! {
        div class="export-dropdown" {
            button type="button" class="btn btn-default" {
                (maud::PreEscaped("<script>me().on('click',function(ev){me(ev).nextElementSibling.classList.toggle('is-open')})</script>"))
                (crate::components::icon::download_icon("w-4 h-4"))
                " 导出"
            }
            div class="export-dropdown-menu" {
                @for btn in menu_buttons {
                    (btn)
                }
            }
        }
    }
}

/// 导出结果区域 HTML 片段（handler 调用）
pub fn render_export_result(task_id: i64, filename: &str) -> Markup {
    let download_path = format!("/excel/export/download/{}", task_id);
    html! {
        div class="export-result" {
            "✓ 导出完成"
            a href=(download_path) class="btn btn-sm btn-primary" download {
                (crate::components::icon::download_icon("w-3.5 h-3.5"))
                " " (filename) ".xlsx"
            }
        }
    }
}
```

- [ ] **Step 2: 在 `components/mod.rs` 中注册**

在 `abt-web/src/components/mod.rs` 末尾追加：

```rust
pub mod import_modal;
pub mod export_button;
```

> 注意：`import_modal` 模块在 Task 4 中创建。这里先声明，编译时会报错。可以先只添加 `export_button`，Task 4 时再添加 `import_modal`。或者先创建一个空的 `import_modal.rs` 占位。

**安全做法**：只添加 `export_button`：

```rust
pub mod export_button;
```

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/components/export_button.rs abt-web/src/components/mod.rs
git commit -m "feat: 通用导出按钮组件 export_button + export_dropdown"
```

---

## Task 4: 通用前端组件 — import_modal.rs

**Files:**
- Create: `abt-web/src/components/import_modal.rs`
- Modify: `abt-web/src/components/mod.rs`（追加 `pub mod import_modal;`）

- [ ] **Step 1: 创建 `import_modal.rs`**

```rust
use maud::{html, Markup};

/// 导入 Modal 配置
pub struct ImportModalConfig {
    pub import_type: &'static str,
    pub title: &'static str,
    pub template_columns: &'static str,
}

/// 渲染导入 Modal（页面底部声明，Surreal.js 控制 is-open）
pub fn import_modal(config: &ImportModalConfig) -> Markup {
    let modal_id = "import-modal";
    let template_path = format!("/excel/template/{}", config.import_type);
    let upload_path = format!("/excel/import/{}", config.import_type);

    html! {
        div id=(modal_id) class="modal-overlay" onclick="hsBackdropClose(this,event,'is-open')" {
            div class="modal" style="max-width:560px" {
                div class="modal-head" {
                    h2 { (config.title) }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        onclick="hsRemoveClosest(this,'.modal-overlay','is-open')" { "×" }
                }
                div class="modal-body" {
                    div id="import-content" {
                        (render_import_form(config))
                    }
                }
            }
        }
        (maud::PreEscaped(format!(r#"<script>
            me('#{}')
                .on('htmx:afterSettle',function(){{}})
                .on('click',function(ev){{if(ev.target===me('#{0}'))me('#{0}').classRemove('is-open')}});
        </script>"#, modal_id)))
    }
}

/// 初始状态：文件选择区 + 模板下载
fn render_import_form(config: &ImportModalConfig) -> Markup {
    let template_path = format!("/excel/template/{}", config.import_type);
    let upload_path = format!("/excel/import/{}", config.import_type);

    html! {
        div class="import-file-zone" {
            p class="import-cols" { "列格式：" (config.template_columns) }
            a href=(template_path) class="btn btn-default" download {
                (crate::components::icon::download_icon("w-4 h-4"))
                " 下载模板"
            }
            form
                hx-post=(upload_path)
                hx-target="#import-content"
                hx-swap="innerHTML"
                hx-encoding="multipart/form-data"
                hx-indicator="#import-content .htmx-indicator" {
                input type="file" name="file" accept=".xlsx,.xls" required;
                div class="import-actions" {
                    button type="submit" class="btn btn-primary" {
                        "开始导入"
                    }
                    div class="htmx-indicator" {
                        "上传中..."
                    }
                }
            }
        }
    }
}

/// 进行中状态：进度条 + 轮询触发器
pub fn render_import_progress(import_type: &str, task_id: i64, current: usize, total: usize) -> Markup {
    let pct = if total > 0 { (current * 100) / total } else { 0 };
    let progress_path = format!("/excel/import/{}/progress/{}", import_type, task_id);

    html! {
        div class="import-progress" {
            p { "正在导入... " (current) "/" (total) }
            div class="import-progress-bar" {
                div class="import-progress-fill" style=(format!("width:{}%", pct)) {}
            }
        }
        div hx-get=(progress_path)
             hx-trigger="every 1s"
             hx-target="#import-content"
             hx-swap="innerHTML" {}
    }
}

/// 完成状态：结果统计 + 错误详情
pub fn render_import_result(result: &abt_core::shared::excel::ImportResult) -> Markup {
    html! {
        div class="import-result" {
            div class="import-result-stats" {
                div class="import-stat" {
                    span class="import-stat-value success" { (result.success_count) }
                    span class="import-stat-label" { "成功" }
                }
                div class="import-stat" {
                    span class="import-stat-value failed" { (result.failed_count) }
                    span class="import-stat-label" { "失败" }
                }
            }
            @if !result.row_errors.is_empty() {
                div class="import-errors" {
                    p style="font-weight:600;margin-bottom:4px" { "错误详情：" }
                    ul {
                        @for err in &result.row_errors {
                            li {
                                "第 " (err.row_index) " 行，列 \"" (err.column_name) "\"："
                                (err.reason)
                                @if let Some(ref v) = err.raw_value {
                                    " (" (v) ")"
                                }
                            }
                        }
                    }
                }
            }
            @if !result.errors.is_empty() {
                div class="import-errors" style="margin-top:8px" {
                    p style="font-weight:600;margin-bottom:4px" { "其他错误：" }
                    ul {
                        @for err in &result.errors {
                            li { (err) }
                        }
                    }
                }
            }
            div style="margin-top:12px;text-align:right" {
                button type="button" class="btn btn-default"
                    onclick="hsRemoveClosest(this,'.modal-overlay','is-open')" { "关闭" }
            }
        }
    }
}
```

- [ ] **Step 2: 在 `components/mod.rs` 中追加**

```rust
pub mod import_modal;
```

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/components/import_modal.rs abt-web/src/components/mod.rs
git commit -m "feat: 通用导入 Modal 组件 import_modal（文件选择 + 进度 + 结果）"
```

---

## Task 5: 后端路由 — excel.rs（TypedPath + Handler + 模板生成）

**Files:**
- Create: `abt-web/src/routes/excel.rs`
- Modify: `abt-web/src/routes/mod.rs`

这是核心任务，包含所有路由和 handler。

- [ ] **Step 1: 创建 `abt-web/src/routes/excel.rs`**

```rust
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use axum::extract::{Multipart, Form};
use axum::response::{Html, IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use abt_core::shared::excel::{
    ImportSource, ImportResult,
    ProductInventoryImporter, LaborProcessImporter,
    ProductAllExporter, ProductWithoutPriceExporter,
    BomExporter, BomsNoLaborCostExporter,
    CategoryExporter, LaborProcessDictExporter, LaborProcessExporter,
    WarehouseLocationExporter,
    ProgressTracker,
};
use abt_core::shared::excel::helpers::write_headers;

use crate::components::import_modal::{render_import_progress, render_import_result};
use crate::components::export_button::render_export_result;
use crate::errors::Result;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/import/:import_type")]
pub struct ExcelImportUploadPath {
    pub import_type: String,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/import/:import_type/progress/:task_id")]
pub struct ExcelImportProgressPath {
    pub import_type: String,
    pub task_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/export/:export_type")]
pub struct ExcelExportStartPath {
    pub export_type: String,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/export/download/:task_id")]
pub struct ExcelExportDownloadPath {
    pub task_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/template/:import_type")]
pub struct ExcelTemplatePath {
    pub import_type: String,
}

// ── Form ──

#[derive(Deserialize)]
pub struct ExportForm {
    pub bom_id: Option<i64>,
    pub product_code: Option<String>,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ExcelImportUploadPath::PATH, post(post_import_upload))
        .route(ExcelImportProgressPath::PATH, get(get_import_progress))
        .route(ExcelExportStartPath::PATH, post(post_export_start))
        .route(ExcelExportDownloadPath::PATH, get(get_export_download))
        .route(ExcelTemplatePath::PATH, get(get_template))
}

// ── Handlers ──

pub async fn post_import_upload(
    _path: ExcelImportUploadPath,
    state: axum::extract::State<AppState>,
    multipart: Multipart,
) -> Result<Html<String>> {
    let import_type = _path.import_type.clone();
    let bytes = extract_file_bytes(multipart).await?;

    let task_id = state.next_task_id();

    // 注册进度跟踪
    state.import_progress.insert(task_id, crate::state::ImportTaskState {
        status: crate::state::TaskStatus::Running,
        current: 0,
        total: 0,
        result: None,
    });

    let pool = state.pool.clone();
    let progress_store = state.import_progress.clone();

    // 后台执行导入
    let import_type_clone = import_type.clone();
    tokio::spawn(async move {
        let tracker = ProgressTracker::new();
        let result = execute_import(&import_type_clone, pool, bytes, tracker.clone(), &progress_store, task_id).await;

        match result {
            Ok(import_result) => {
                progress_store.insert(task_id, crate::state::ImportTaskState {
                    status: crate::state::TaskStatus::Completed,
                    current: tracker.snapshot().total,
                    total: tracker.snapshot().total,
                    result: Some(import_result),
                });
            }
            Err(e) => {
                let err_result = ImportResult {
                    errors: vec![e.to_string()],
                    ..Default::default()
                };
                progress_store.insert(task_id, crate::state::ImportTaskState {
                    status: crate::state::TaskStatus::Failed,
                    current: 0,
                    total: 0,
                    result: Some(err_result),
                });
            }
        }
    });

    // 立即返回进度页面
    let html = render_import_progress(&import_type, task_id, 0, 0);
    Ok(Html(html.into_string()))
}

async fn execute_import(
    import_type: &str,
    pool: sqlx::PgPool,
    bytes: Vec<u8>,
    tracker: std::sync::Arc<ProgressTracker>,
    progress_store: &std::sync::Arc<dashmap::DashMap<i64, crate::state::ImportTaskState>>,
    task_id: i64,
) -> anyhow::Result<ImportResult> {
    let source = ImportSource::Bytes(bytes);

    let result = match import_type {
        "product-inventory" => {
            let importer = ProductInventoryImporter::new(pool, tracker.clone());
            importer.import(source).await?
        }
        "labor-process" => {
            let importer = LaborProcessImporter::new(pool, tracker.clone());
            importer.import(source).await?
        }
        "warehouse-location" => {
            abt_core::shared::excel::import_warehouse_locations(&pool, source).await?
        }
        _ => return Err(anyhow::anyhow!("未知的导入类型: {}", import_type)),
    };

    // 将 ImportResult 或 LaborProcessImportResult 转换为 ImportResult
    Ok(result)
}

pub async fn get_import_progress(
    path: ExcelImportProgressPath,
    state: axum::extract::State<AppState>,
) -> Result<Html<String>> {
    let task = state.import_progress.get(&path.task_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::NotFound("任务不存在".into()))?;

    match task.status {
        crate::state::TaskStatus::Running => {
            let html = render_import_progress(&path.import_type, path.task_id, task.current, task.total);
            Ok(Html(html.into_string()))
        }
        crate::state::TaskStatus::Completed | crate::state::TaskStatus::Failed => {
            let result = task.result.clone().unwrap_or_default();
            let html = render_import_result(&result);
            Ok(Html(html.into_string()))
        }
    }
}

pub async fn post_export_start(
    path: ExcelExportStartPath,
    state: axum::extract::State<AppState>,
    form: Option<Form<ExportForm>>,
) -> Result<Html<String>> {
    let pool = state.pool.clone();
    let params = form.map(|f| f.0).unwrap_or(ExportForm {
        bom_id: None,
        product_code: None,
    });

    let (bytes, filename) = match path.export_type.as_str() {
        "product-all" => {
            let exporter = ProductAllExporter::new(pool);
            (exporter.export().await?, "产品库存导出".to_string())
        }
        "product-without-price" => {
            let exporter = ProductWithoutPriceExporter::new(pool);
            (exporter.export().await?, "无价格产品".to_string())
        }
        "bom" => {
            let bom_id = params.bom_id
                .ok_or_else(|| abt_core::shared::types::DomainError::Validation("缺少 bom_id".into()))?;
            let exporter = BomExporter::new(pool, bom_id);
            let (data, name) = exporter.export_with_name().await?;
            (data, name)
        }
        "bom-no-labor-cost" => {
            let exporter = BomsNoLaborCostExporter::new(pool);
            (exporter.export().await?, "缺少人工成本BOM".to_string())
        }
        "category" => {
            let exporter = CategoryExporter::new(pool);
            (exporter.export().await?, "分类导出".to_string())
        }
        "labor-process-dict" => {
            let exporter = LaborProcessDictExporter::new(pool);
            (exporter.export().await?, "工序字典导出".to_string())
        }
        "labor-process" => {
            let code = params.product_code
                .ok_or_else(|| abt_core::shared::types::DomainError::Validation("缺少产品编码".into()))?;
            let exporter = LaborProcessExporter::new(pool, code);
            (exporter.export().await?, "工艺路线导出".to_string())
        }
        "warehouse-location" => {
            let exporter = WarehouseLocationExporter::new(pool);
            (exporter.export().await?, "仓库库位导出".to_string())
        }
        _ => return Err(abt_core::shared::types::DomainError::NotFound(
            format!("未知的导出类型: {}", path.export_type)
        ).into()),
    };

    let task_id = state.store_export_file(bytes, &filename);
    let html = render_export_result(task_id, &filename);
    Ok(Html(html.into_string()))
}

pub async fn get_export_download(
    path: ExcelExportDownloadPath,
    state: axum::extract::State<AppState>,
) -> Result<Response> {
    let info = state.get_export_file(path.task_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::NotFound("文件不存在或已过期".into()))?;

    let body = axum::body::Body::from(info.bytes);
    let response = Response::builder()
        .header("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        .header("Content-Disposition", format!("attachment; filename=\"{}.xlsx\"", info.filename))
        .body(body)
        .unwrap();

    Ok(response)
}

pub async fn get_template(
    path: ExcelTemplatePath,
) -> Result<Response> {
    let (bytes, filename) = generate_template(&path.import_type)?;
    let body = axum::body::Body::from(bytes);
    let response = Response::builder()
        .header("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        .header("Content-Disposition", format!("attachment; filename=\"{}.xlsx\"", filename))
        .body(body)
        .unwrap();
    Ok(response)
}

// ── Helpers ──

async fn extract_file_bytes(mut multipart: Multipart) -> Result<Vec<u8>> {
    let field = multipart.next_field().await
        .map_err(|e| abt_core::shared::types::DomainError::Validation(format!("上传失败: {}", e)))?
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("未找到上传文件".into()))?;

    let filename = field.file_name().unwrap_or("").to_string();
    if !filename.ends_with(".xlsx") && !filename.ends_with(".xls") {
        return Err(abt_core::shared::types::DomainError::Validation("仅支持 Excel 文件（.xlsx / .xls）".into()).into());
    }

    let bytes = field.bytes().await
        .map_err(|e| abt_core::shared::types::DomainError::Validation(format!("读取文件失败: {}", e)))?;

    if bytes.len() > 10 * 1024 * 1024 {
        return Err(abt_core::shared::types::DomainError::Validation("文件大小不能超过 10MB".into()).into());
    }

    Ok(bytes.to_vec())
}

fn generate_template(import_type: &str) -> Result<(Vec<u8>, String)> {
    use rust_xlsxwriter::Workbook;

    let (sheet_name, columns, filename) = match import_type {
        "product-inventory" => (
            "产品导入",
            &["新编码", "旧编码", "物料名称", "库位编码", "库存数量", "价格", "安全库存", "分类ID"][..],
            "产品导入模板",
        ),
        "labor-process" => (
            "工艺路线导入",
            &["产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注"][..],
            "工艺路线导入模板",
        ),
        "warehouse-location" => (
            "仓库库位导入",
            &["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"][..],
            "仓库库位导入模板",
        ),
        _ => return Err(abt_core::shared::types::DomainError::NotFound(
            format!("未知的导入类型: {}", import_type)
        ).into()),
    };

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet().set_name(sheet_name)
        .map_err(|e| anyhow::anyhow!("创建工作表失败: {}", e))?;

    write_headers(&mut worksheet.clone(), columns)
        .map_err(|e| anyhow::anyhow!("写入表头失败: {}", e))?;

    // 设置列宽
    for (col, header) in columns.iter().enumerate() {
        let _ = worksheet.set_column_width(col as u16, (header.len() as u16 + 4).max(12));
    }

    let bytes = workbook.save_to_buffer()
        .map_err(|e| anyhow::anyhow!("生成模板失败: {}", e))?;

    Ok((bytes.to_vec(), filename.to_string()))
}
```

> **注意**：`LaborProcessImporter::import()` 返回 `LaborProcessImportResult`，不是 `ImportResult`。需要在 `execute_import` 中做类型转换。同样，`import_warehouse_locations` 返回 `ImportResult`。具体转换逻辑需要查看 `LaborProcessImportResult` 的定义。如果它实现了 `Into<ImportResult>` 或包含相同字段，可直接映射。

- [ ] **Step 2: 在 `abt-web/Cargo.toml` 中确认依赖**

确保 `abt-web/Cargo.toml` 中有（已在 workspace 中）：

```toml
rust_xlsxwriter = { workspace = true }
calamine = { workspace = true }
```

如果没有，需要添加。检查根 `Cargo.toml` 的 `[workspace.dependencies]` 是否有这些 crate。

- [ ] **Step 3: 在 `routes/mod.rs` 中注册 excel 路由**

在 `abt-web/src/routes/mod.rs` 中：

1. 顶部 `pub mod` 块中追加：`pub mod excel;`
2. 在 `router()` 函数的 `.merge()` 链中，在 `// ── Master Data (MD) ──` 区域后追加：

```rust
                // ── Excel Import/Export ──
                .merge(excel::router())
```

- [ ] **Step 4: 运行 `cargo check` 修复编译错误**

```bash
cd abt-web && cargo check 2>&1 | head -30
```

Expected: 可能存在类型不匹配（`LaborProcessImportResult` vs `ImportResult`）。根据编译器提示修复。

可能需要的修复：
- `LaborProcessImporter::import()` 返回 `Result<LaborProcessImportResult>`，需要手动转换为 `ImportResult`
- `write_headers` 接受 `&mut Worksheet`，可能需要调整可变性

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/routes/excel.rs abt-web/src/routes/mod.rs abt-web/Cargo.toml
git commit -m "feat: 导入导出路由和 Handler — TypedPath + 分发逻辑 + 模板生成"
```

---

## Task 6: 产品模块集成

**Files:**
- Modify: `abt-web/src/pages/product_list.rs`

这是第一个集成点，验证完整流程。

- [ ] **Step 1: 在 `product_list.rs` 顶部追加 import**

```rust
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button::{self, ExportItem};
```

- [ ] **Step 2: 修改 `product_list_page()` 函数的 page-actions 区域**

在 `div class="page-actions"` 内，在 `@if can_create` 之前插入导入/导出按钮：

```rust
                div class="page-actions" {
                    // ── 导入按钮 ──
                    button type="button" class="btn btn-default"
                        onclick="hsAdd(null,'#import-modal','is-open')" {
                        (icon::upload_icon("w-4 h-4"))
                        "导入"
                    }
                    // ── 导出下拉菜单 ──
                    (export_button::export_dropdown(&[
                        ExportItem { label: "含库存产品", export_type: "product-all" },
                        ExportItem { label: "不含价格产品", export_type: "product-without-price" },
                    ]))
                    // ── 原有按钮 ──
                    @if can_create {
                        a href=(ProductCreatePath::PATH) class="btn btn-primary" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建产品"
                        }
                    }
                }
```

- [ ] **Step 3: 在 `product_list_page()` 的 html! 宏末尾、最终 `}` 之前，追加 Modal 和导出结果区域**

在 BOM 引用 Drawer 之后、外层 `div` 关闭之前：

```rust
            // ── Import Modal ──
            (import_modal::import_modal(&ImportModalConfig {
                import_type: "product-inventory",
                title: "导入产品库存",
                template_columns: "新编码, 旧编码, 物料名称, 库位编码, 库存数量, 价格, 安全库存, 分类ID",
            }))

            // ── Export Result ──
            div id="export-result" {}
```

- [ ] **Step 4: 运行 `cargo check` 验证**

```bash
cd abt-web && cargo check 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/product_list.rs
git commit -m "feat(product): 产品列表页添加导入/导出按钮和 Modal"
```

---

## Task 7: BOM 模块集成

**Files:**
- Modify: `abt-web/src/pages/bom_list.rs`
- Modify: `abt-web/src/pages/bom_detail.rs`

- [ ] **Step 1: 在 `bom_list.rs` 顶部追加 import**

```rust
use crate::components::export_button::{self, ExportItem};
```

- [ ] **Step 2: 找到 `bom_list` 的 `page-actions` 区域，在原有按钮之前添加导出下拉菜单**

```rust
                div class="page-actions" {
                    (export_button::export_dropdown(&[
                        ExportItem { label: "缺少人工成本BOM", export_type: "bom-no-labor-cost" },
                    ]))
                    // 原有按钮...
                }
```

- [ ] **Step 3: 在 `bom_list` 页面的 html! 宏末尾追加导出结果区域**

```rust
            // ── Export Result ──
            div id="export-result" {}
```

- [ ] **Step 4: 在 `bom_detail.rs` 顶部追加 import**

```rust
use crate::components::export_button;
```

- [ ] **Step 5: 在 `bom_detail.rs` 的 `page-actions` 区域，在已有按钮之后添加 BOM 导出按钮**

需要获取当前 `bom_id`，在 `page-actions` 的 div 内追加：

```rust
                    button type="button" class="btn btn-default"
                        hx-post=(format!("/excel/export/bom"))
                        hx-vals=(format!("{{\"bom_id\": {}}}", bom_id))
                        hx-target="#export-result"
                        hx-swap="innerHTML"
                        hx-indicator="#export-result" {
                        (icon::download_icon("w-4 h-4"))
                        "导出 BOM"
                    }
```

> 注意：`bom_id` 变量名需根据 `bom_detail.rs` 的实际上下文确认。搜索该文件中已有的 `bom_id` 变量名。

- [ ] **Step 6: 在 `bom_detail.rs` 的页面 html! 末尾追加导出结果区域**

```rust
            div id="export-result" {}
```

- [ ] **Step 7: 运行 `cargo check` 验证**

```bash
cd abt-web && cargo check 2>&1 | tail -5
```

- [ ] **Step 8: Commit**

```bash
git add abt-web/src/pages/bom_list.rs abt-web/src/pages/bom_detail.rs
git commit -m "feat(bom): BOM 列表页和详情页添加导出按钮"
```

---

## Task 8: 工艺模块集成

**Files:**
- Modify: `abt-web/src/pages/labor_process_dict_list.rs`
- Modify: `abt-web/src/pages/routing_list.rs`

- [ ] **Step 1: 在 `labor_process_dict_list.rs` 顶部追加 import**

```rust
use crate::components::export_button;
```

- [ ] **Step 2: 在 `process_dict_list_page()` 的 `page-actions` 中添加导出按钮**

在 `@if can_create` 之前：

```rust
                div class="page-actions" {
                    (export_button::export_button("导出工序字典", "labor-process-dict"))
                    @if can_create {
                        a class="btn btn-primary" href=(ProcessDictCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建工序"
                        }
                    }
                }
```

- [ ] **Step 3: 在 `labor_process_dict_list` 页面末尾追加导出结果区域**

```rust
            div id="export-result" {}
```

- [ ] **Step 4: 在 `routing_list.rs` 顶部追加 import**

```rust
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button;
```

- [ ] **Step 5: 在 `routing_list` 的 `page-actions` 中添加导入和导出按钮**

```rust
                div class="page-actions" {
                    button type="button" class="btn btn-default"
                        onclick="hsAdd(null,'#import-modal','is-open')" {
                        (icon::upload_icon("w-4 h-4"))
                        "导入"
                    }
                    (export_button::export_button("导出工艺路线", "labor-process"))
                    // 原有按钮...
                }
```

- [ ] **Step 6: 在 `routing_list` 页面末尾追加 Modal 和导出结果区域**

```rust
            (import_modal::import_modal(&ImportModalConfig {
                import_type: "labor-process",
                title: "导入工艺路线",
                template_columns: "产品编码, 工序编码, 工序名称, 单价, 数量, 排序, 备注",
            }))
            div id="export-result" {}
```

- [ ] **Step 7: 运行 `cargo check` 验证**

```bash
cd abt-web && cargo check 2>&1 | tail -5
```

- [ ] **Step 8: Commit**

```bash
git add abt-web/src/pages/labor_process_dict_list.rs abt-web/src/pages/routing_list.rs
git commit -m "feat(labor): 工序字典添加导出，工艺路线添加导入导出"
```

---

## Task 9: 仓库库位模块集成

**Files:**
- Modify: `abt-web/src/pages/wms_warehouse_list.rs`

- [ ] **Step 1: 在 `wms_warehouse_list.rs` 顶部追加 import**

```rust
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button;
```

- [ ] **Step 2: 在 `warehouse_list_page()` 的 `page-actions` 中添加导入/导出按钮**

在 `@if can_create` 之前：

```rust
                div class="page-actions" {
                    button type="button" class="btn btn-default"
                        onclick="hsAdd(null,'#import-modal','is-open')" {
                        (icon::upload_icon("w-4 h-4"))
                        "导入"
                    }
                    (export_button::export_button("导出库位", "warehouse-location"))
                    @if can_create {
                        a class="btn btn-primary" href=(WarehouseCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建仓库"
                        }
                    }
                }
```

- [ ] **Step 3: 在页面末尾追加 Modal 和导出结果区域**

```rust
            (import_modal::import_modal(&ImportModalConfig {
                import_type: "warehouse-location",
                title: "导入仓库库位",
                template_columns: "仓库编码, 仓库名称, 库位编码, 库位名称, 容量",
            }))
            div id="export-result" {}
```

- [ ] **Step 4: 运行 `cargo check` 验证**

```bash
cd abt-web && cargo check 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/wms_warehouse_list.rs
git commit -m "feat(wms): 仓库管理页添加导入/导出按钮"
```

---

## Task 10: 编译修复 + 端到端验证

**Files:**
- 可能修改任何 Task 5-9 中创建/修改的文件

- [ ] **Step 1: 运行 `cargo clippy` 全面检查**

```bash
cd abt-web && cargo clippy 2>&1 | head -40
```

- [ ] **Step 2: 修复所有 clippy 警告和错误**

常见问题预期：
- `LaborProcessImportResult` vs `ImportResult` 类型不匹配
- `write_headers` 的 `&mut Worksheet` 可变性
- 未使用的 import 警告
- `state.pool` 的 clone 需要 `sqlx::PgPool` 类型

- [ ] **Step 3: 再次运行 `cargo clippy` 确认通过**

```bash
cd abt-web && cargo clippy 2>&1 | tail -3
```

Expected: 无错误无警告

- [ ] **Step 4: Commit 修复**

```bash
git add -A
git commit -m "fix: 编译修复和 clippy 警告清理"
```

---

## Task 11: 设计文档同步

**Files:**
- Modify: `docs/uml-design/00-shared-infrastructure.html`（如适用）

- [ ] **Step 1: 更新 `docs/uml-design/` 中 Excel 服务相关的设计文档**

在共享基础设施设计文档中补充 Web handler 层的接口说明：
- TypedPath 路由定义
- Handler 分发逻辑
- 进度跟踪机制（内存 ProgressStore）

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/
git commit -m "docs: 同步 Excel Web handler 设计文档"
```
