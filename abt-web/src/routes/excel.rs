//! Excel 导入导出路由
//!
//! 提供文件上传、进度轮询、模板下载、导出下载等通用接口。
//! 使用 TypedPath 强类型路由 + RequestContext 权限校验。

use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::components::import_modal::{render_import_progress, render_import_result};
use crate::errors::{Result as WebResult, WebError};
use crate::state::AppState;
use crate::toast::{add_toast, ToastType};
use crate::utils::RequestContext;
use abt_core::shared::excel::types::{ImportResult, ImportSource, RowError};
use abt_core::shared::excel::helpers::write_headers;
use abt_core::shared::types::PgPool;
use abt_macros::require_permission;

// ── 导出表单 ──

#[derive(Deserialize, Default)]
pub struct ExportForm {
    pub bom_id: Option<i64>,
    pub product_code: Option<String>,
}

// ── TypedPath 路由定义 ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/import/{import_type}")]
pub struct ExcelImportUploadPath {
    pub import_type: String,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/import/{import_type}/progress/{task_id}")]
pub struct ExcelImportProgressPath {
    pub import_type: String,
    pub task_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/export/{export_type}")]
pub struct ExcelExportStartPath {
    pub export_type: String,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/export/download/{task_id}")]
pub struct ExcelExportDownloadPath {
    pub task_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/excel/template/{import_type}")]
pub struct ExcelTemplatePath {
    pub import_type: String,
}

// ── 路径常量（供前端组件生成 URL） ──
#[allow(dead_code)]
pub const IMPORT_UPLOAD_PATH: &str = "/excel/import";
pub const EXPORT_START_PATH: &str = "/excel/export";
pub const EXPORT_DOWNLOAD_PATH: &str = "/excel/export/download";
#[allow(dead_code)]
pub const TEMPLATE_PATH: &str = "/excel/template";

// ── 路由注册 ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ExcelImportUploadPath::PATH, post(post_import_upload))
        .route(ExcelImportProgressPath::PATH, get(get_import_progress))
        .route(ExcelExportStartPath::PATH, post(post_export_start))
        .route(ExcelExportDownloadPath::PATH, get(get_export_download))
        .route(ExcelTemplatePath::PATH, get(get_template))
}

// ── Helper: 构建 Excel 下载响应头 ──

fn excel_download_headers(filename: &str) -> HeaderMap {
    let safe = sanitize_filename(filename);
    let encoded = percent_encode(&format!("{}.xlsx", safe));
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"download.xlsx\"; filename*=UTF-8''{}", encoded)
            .parse()
            .unwrap(),
    );
    headers
}

/// 清理文件名中的危险字符（防止 header 注入）
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c == '"' || c == '\r' || c == '\n' || c == '\\' { '_' } else { c })
        .collect()
}

/// 百分号编码（RFC 3986 unreserved + 允许 UTF-8 percent-encoding）
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

// ── Handler: 导入上传 ──

#[require_permission("PRODUCT", "create")]
pub async fn post_import_upload(
    path: ExcelImportUploadPath,
    ctx: RequestContext,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> WebResult<impl IntoResponse> {
    let import_type = path.import_type.clone();

    // 并发限制：快速检查是否有余量（非精确，但避免 spawn 后无限等待）
    let semaphore = state.import_semaphore.clone();
    if semaphore.available_permits() == 0 {
        return Err(WebError::from(abt_core::shared::types::DomainError::Validation(
            "当前导入任务过多，请稍后再试".into(),
        )));
    }

    // 提取文件
    let bytes = extract_file_bytes(&mut multipart)
        .await
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    let task_id = state.next_task_id();
    let user_id = ctx.claims.sub;
    let pool = state.pool.clone();
    let import_progress = state.import_progress.clone();
    let import_type_for_spawn = import_type.clone();
    let now = chrono::Utc::now();

    // 异步执行导入
    tokio::spawn(async move {
        // 在 spawn 内获取信号量，持有直到导入完成
        let _permit = match semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => return, // semaphore 已关闭
        };

        let tracker = abt_core::shared::excel::helpers::ProgressTracker::new();
        tracker.set_total(100);

        // 初始化进度状态
        import_progress.insert(task_id, crate::state::ImportTaskState {
            status: crate::state::TaskStatus::Running,
            current: tracker.snapshot().current,
            total: tracker.snapshot().total,
            result: None,
            user_id,
            created_at: now,
        });

        let result = execute_import(&pool, &import_type_for_spawn, ImportSource::Bytes(bytes), tracker.clone()).await;

        // 更新最终状态
        let final_state = match result {
            Ok(import_result) => crate::state::ImportTaskState {
                status: crate::state::TaskStatus::Completed,
                current: tracker.snapshot().current,
                total: tracker.snapshot().total,
                result: Some(import_result),
                user_id,
                created_at: now,
            },
            Err(e) => {
                tracing::error!("Import task {} failed: {}", task_id, e);
                crate::state::ImportTaskState {
                    status: crate::state::TaskStatus::Failed,
                    current: tracker.snapshot().current,
                    total: tracker.snapshot().total,
                    result: Some(ImportResult {
                        success_count: 0,
                        failed_count: 0,
                        errors: vec![e.to_string()],
                        row_errors: vec![],
                    }),
                    user_id,
                    created_at: now,
                }
            }
        };
        import_progress.insert(task_id, final_state);
    });

    Ok(render_import_progress(&import_type, task_id, 0, 100))
}

// ── Handler: 导入进度轮询 ──

pub async fn get_import_progress(
    path: ExcelImportProgressPath,
    ctx: RequestContext,
    State(state): State<AppState>,
) -> WebResult<impl IntoResponse> {
    let user_id = ctx.claims.sub;

    let task_state = state
        .import_progress
        .get(&path.task_id)
        .filter(|r| r.value().user_id == user_id)
        .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::NotFound(
            format!("任务 {} 不存在", path.task_id),
        )))?;

    match &task_state.status {
        crate::state::TaskStatus::Running => {
            Ok(render_import_progress(&path.import_type, path.task_id, task_state.current, task_state.total))
        }
        crate::state::TaskStatus::Completed | crate::state::TaskStatus::Failed => {
            let result = task_state.result.as_ref()
                .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::Internal(
                    anyhow::anyhow!("任务完成但结果缺失"),
                )))?;
            Ok(render_import_result(result))
        }
    }
}

// ── Handler: 导出启动 ──

pub async fn post_export_start(
    path: ExcelExportStartPath,
    ctx: RequestContext,
    Query(form): Query<ExportForm>,
) -> WebResult<impl IntoResponse> {
    // 根据 export_type 检查对应资源权限
    let (resource, action) = match path.export_type.as_str() {
        "warehouse-location" => ("WAREHOUSE", "read"),
        _ => ("PRODUCT", "read"),
    };
    crate::permissions::check_permission(&ctx, resource, action).await?;

    let pool = ctx.state.pool.clone();
    let user_id = ctx.claims.sub;

    let (bytes, filename) = execute_export(&pool, &path.export_type, form)
        .await
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    let safe_name = sanitize_filename(&filename);
    let task_id = ctx.state.store_export_file(bytes, &safe_name, user_id);
    let download_url = format!("{}/{}", EXPORT_DOWNLOAD_PATH, task_id);
    add_toast(user_id, "导出完成", ToastType::Success);
    let trigger = format!("{{\"showToast\":{{}},\"exportDone\":{{\"url\":\"{}\"}}}}", download_url);

    Ok(([("HX-Trigger", trigger)], ()))
}

// ── Handler: 导出下载 ──

pub async fn get_export_download(
    path: ExcelExportDownloadPath,
    ctx: RequestContext,
    State(state): State<AppState>,
) -> WebResult<impl IntoResponse> {
    let user_id = ctx.claims.sub;

    let file_info = state.get_export_file(path.task_id, user_id)
        .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::NotFound(
            format!("导出文件 {} 不存在或已过期", path.task_id),
        )))?;

    Ok((excel_download_headers(&file_info.filename), file_info.bytes).into_response())
}

// ── Handler: 模板下载 ──

pub async fn get_template(
    path: ExcelTemplatePath,
) -> WebResult<impl IntoResponse> {
    let bytes = generate_template(&path.import_type)
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    let filename = sanitize_filename(&format!("{}_template", path.import_type));
    Ok((excel_download_headers(&filename), bytes).into_response())
}

// ── Helper: 执行导入 ──

async fn execute_import(
    pool: &PgPool,
    import_type: &str,
    source: ImportSource,
    tracker: Arc<abt_core::shared::excel::helpers::ProgressTracker>,
) -> anyhow::Result<ImportResult> {
    match import_type {
        "product-inventory" => {
            let importer = abt_core::shared::excel::product_inventory_import::ProductInventoryImporter::new(pool.clone(), tracker);
            importer.import(source).await
        }
        "labor-process" => {
            let importer = abt_core::shared::excel::labor_process_import::LaborProcessImporter::new(pool.clone(), tracker);
            let lp_result = importer.import(source).await?;
            Ok(convert_labor_process_result(lp_result))
        }
        "warehouse-location" => {
            abt_core::shared::excel::warehouse_location_import::import_warehouse_locations(pool, source).await
        }
        _ => anyhow::bail!("未知的导入类型: {}", import_type),
    }
}

fn convert_labor_process_result(lp_result: abt_core::shared::excel::labor_process_import::LaborProcessImportResult) -> ImportResult {
    let mut errors = Vec::new();
    let mut row_errors = Vec::new();

    for r in &lp_result.results {
        if !r.error_message.is_empty() {
            errors.push(format!("第{}行 {}: {}", r.row_number, r.process_name, r.error_message));
            row_errors.push(RowError {
                row_index: r.row_number as usize,
                column_name: r.process_name.clone(),
                reason: r.error_message.clone(),
                raw_value: None,
            });
        }
    }

    ImportResult {
        success_count: lp_result.success_count as usize,
        failed_count: lp_result.failure_count as usize,
        errors,
        row_errors,
    }
}

// ── Helper: 执行导出 ──

async fn execute_export(
    pool: &PgPool,
    export_type: &str,
    form: ExportForm,
) -> anyhow::Result<(Vec<u8>, String)> {
    match export_type {
        "bom" => {
            let bom_id = form.bom_id.ok_or_else(|| anyhow::anyhow!("BOM导出需要 bom_id 参数"))?;
            let exporter = abt_core::shared::excel::bom_export::BomExporter::new(pool.clone(), bom_id);
            let (bytes, name) = exporter.export_with_name().await?;
            Ok((bytes, name))
        }
        "product-all" => {
            let exporter = abt_core::shared::excel::product_all_export::ProductAllExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "产品清单".to_string()))
        }
        "product-without-price" => {
            let exporter = abt_core::shared::excel::product_without_price_export::ProductWithoutPriceExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "产品清单(不含价格)".to_string()))
        }
        "category" => {
            let exporter = abt_core::shared::excel::category_export::CategoryExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "产品分类".to_string()))
        }
        "labor-process-dict" => {
            let exporter = abt_core::shared::excel::labor_process_dict_export::LaborProcessDictExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "工序字典".to_string()))
        }
        "labor-process" => {
            let exporter = abt_core::shared::excel::labor_process_export::LaborProcessExporter::new(
                pool.clone(),
                form.product_code.unwrap_or_default(),
            );
            let bytes = exporter.export().await?;
            Ok((bytes, "工序清单".to_string()))
        }
        "warehouse-location" => {
            let exporter = abt_core::shared::excel::warehouse_location_export::WarehouseLocationExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "仓库库位".to_string()))
        }
        "boms-list" => {
            let exporter = abt_core::shared::excel::boms_list_export::BomsListExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "BOM清单".to_string()))
        }
        "boms-no-labor-cost" => {
            let exporter = abt_core::shared::excel::boms_no_labor_cost_export::BomsNoLaborCostExporter::new(pool.clone());
            let bytes = exporter.export().await?;
            Ok((bytes, "BOM清单(无人工成本)".to_string()))
        }
        _ => anyhow::bail!("未知的导出类型: {}", export_type),
    }
}

// ── Helper: 提取文件字节 ──

async fn extract_file_bytes(multipart: &mut Multipart) -> anyhow::Result<Vec<u8>> {
    const MAX_SIZE: usize = 10 * 1024 * 1024; // 10MB

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("");
        if name != "file" {
            continue;
        }

        let filename = field.file_name().unwrap_or("");
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // 只接受 .xlsx（移除 .xls，因为解析器只支持 ZIP-based .xlsx）
        if !ext.eq_ignore_ascii_case("xlsx") {
            anyhow::bail!("仅支持 .xlsx 格式");
        }

        let bytes = field.bytes().await?;

        if bytes.len() > MAX_SIZE {
            anyhow::bail!("文件大小不能超过 10MB");
        }

        return Ok(bytes.to_vec());
    }

    anyhow::bail!("未找到文件字段");
}

// ── Helper: 生成模板 ──

fn generate_template(import_type: &str) -> anyhow::Result<Vec<u8>> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet();

    let headers = match import_type {
        "product-inventory" => vec!["新编码", "旧编码", "物料名称", "库位编码", "库存数量", "价格", "安全库存", "分类ID"],
        "labor-process" => vec!["产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注"],
        "warehouse-location" => vec!["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"],
        _ => anyhow::bail!("未知的导入类型: {}", import_type),
    };

    write_headers(worksheet, &headers)?;

    let bytes = workbook.save_to_buffer()?;
    Ok(bytes)
}
