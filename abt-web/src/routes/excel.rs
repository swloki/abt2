//! Excel 导入导出路由
//!
//! 提供文件上传、进度轮询、模板下载、导出下载等通用接口。

use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;

use crate::components::export_button::render_export_result;
use crate::components::import_modal::{render_import_progress, render_import_result};
use crate::errors::{Result as WebResult, WebError};
use crate::state::AppState;
use abt_core::shared::excel::types::{ImportResult, ImportSource, RowError};
use abt_core::shared::excel::helpers::write_headers;
use abt_core::shared::types::PgPool;

/// 构建 Excel 文件下载的 HTTP 响应头
fn excel_download_headers(filename: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename).parse().unwrap(),
    );
    headers
}

// ── 导出表单 ──

#[derive(serde::Deserialize)]
pub struct ExportForm {
    pub bom_id: Option<i64>,
    pub product_code: Option<String>,
}

// ── 路由注册 ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/excel/import/:import_type", post(post_import_upload))
        .route("/excel/import/:import_type/progress/:task_id", get(get_import_progress))
        .route("/excel/export/:export_type", post(post_export_start))
        .route("/excel/export/download/:task_id", get(get_export_download))
        .route("/excel/template/:import_type", get(get_template))
}

// ── Handler: 导入上传 ──

pub async fn post_import_upload(
    Path(import_type): Path<String>,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> WebResult<impl IntoResponse> {

    // 提取文件
    let bytes = extract_file_bytes(&mut multipart)
        .await
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    // 生成 task_id
    let task_id = state.next_task_id();

    let pool = state.pool.clone();
    let import_progress = state.import_progress.clone();
    let import_type_for_spawn = import_type.clone();

    // 异步执行导入（spawn 内统一管理状态写入）
    tokio::spawn(async move {
        let tracker = abt_core::shared::excel::helpers::ProgressTracker::new();

        // 设置总行数（预估，实际由 importer 调整）
        tracker.set_total(100);

        // 初始化进度状态
        import_progress.insert(
            task_id,
            crate::state::ImportTaskState {
                status: crate::state::TaskStatus::Running,
                current: tracker.snapshot().current,
                total: tracker.snapshot().total,
                result: None,
            },
        );

        let result = execute_import(&pool, &import_type_for_spawn, ImportSource::Bytes(bytes), tracker.clone()).await;

        // 更新最终状态（失败时构造包含错误信息的 ImportResult，避免 result=None 导致二次 500）
        let final_state = match result {
            Ok(import_result) => crate::state::ImportTaskState {
                status: crate::state::TaskStatus::Completed,
                current: tracker.snapshot().current,
                total: tracker.snapshot().total,
                result: Some(import_result),
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
                }
            }
        };
        import_progress.insert(task_id, final_state);
    });

    // 立即返回进度 HTML
    Ok(render_import_progress(&import_type, task_id, 0, 100))
}

// ── Handler: 导入进度轮询 ──

pub async fn get_import_progress(
    Path((import_type, task_id)): Path<(String, i64)>,
    State(state): State<AppState>,
) -> WebResult<impl IntoResponse> {

    let task_state = state
        .import_progress
        .get(&task_id)
        .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::NotFound(
            format!("任务 {} 不存在", task_id),
        )))?;

    match &task_state.status {
        crate::state::TaskStatus::Running => {
            let current = task_state.current;
            let total = task_state.total;
            Ok(render_import_progress(&import_type, task_id, current, total))
        }
        crate::state::TaskStatus::Completed | crate::state::TaskStatus::Failed => {
            let result = task_state
                .result
                .as_ref()
                .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::Internal(
                    anyhow::anyhow!("任务完成但结果缺失"),
                )))?;
            Ok(render_import_result(result))
        }
    }
}

// ── Handler: 导出启动 ──

pub async fn post_export_start(
    Path(export_type): Path<String>,
    State(state): State<AppState>,
    form: axum::extract::Form<ExportForm>,
) -> WebResult<impl IntoResponse> {
    let pool = state.pool.clone();

    let (bytes, filename) = execute_export(&pool, &export_type, form.0)
        .await
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    let task_id = state.store_export_file(bytes, &filename);

    Ok(render_export_result(task_id, &filename))
}

// ── Handler: 导出下载 ──

pub async fn get_export_download(
    Path(task_id): Path<i64>,
    State(state): State<AppState>,
) -> WebResult<impl IntoResponse> {

    let file_info = state
        .get_export_file(task_id)
        .ok_or_else(|| WebError::from(abt_core::shared::types::DomainError::NotFound(
            format!("导出文件 {} 不存在或已过期", task_id),
        )))?;

    Ok((excel_download_headers(&file_info.filename), file_info.bytes).into_response())
}

// ── Handler: 模板下载 ──

pub async fn get_template(
    Path(import_type): Path<String>,
) -> WebResult<impl IntoResponse> {

    let bytes = generate_template(&import_type)
        .map_err(|e| WebError::from(abt_core::shared::types::DomainError::Internal(e)))?;

    let filename = format!("{}_template.xlsx", import_type);

    Ok((excel_download_headers(&filename), bytes).into_response())
}

// ── Helper: 执行导入（分发到不同 importer）──

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
            // 转换 LaborProcessImportResult → ImportResult
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

// ── Helper: 执行导出（分发到不同 exporter）──

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

        if ext != "xlsx" && ext != "xls" {
            anyhow::bail!("仅支持 .xlsx 和 .xls 格式");
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
