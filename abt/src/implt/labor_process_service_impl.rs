//! 劳务工序服务实现

use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use calamine::{RangeDeserializerBuilder, Reader, Xlsx, open_workbook};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use serde::Deserialize;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{BomRepo, Executor, LaborProcessDictRepo, LaborProcessRepo};
use crate::service::{LaborProcessService, RoutingService};

pub struct LaborProcessServiceImpl {
    pool: PgPool,
}

impl LaborProcessServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LaborProcessService for LaborProcessServiceImpl {
    // ========================================================================
    // 查询
    // ========================================================================

    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessRepo::find_by_product_code(
            &self.pool, &query.product_code, kw, page, page_size,
        )
        .await?;
        let total = LaborProcessRepo::count_by_product_code(
            &self.pool, &query.product_code, kw,
        )
        .await?;
        Ok((items, total))
    }

    // ========================================================================
    // 写入
    // ========================================================================

    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64> {
        LaborProcessRepo::insert(
            executor,
            &req.product_code,
            req.process_code.as_deref(),
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref(),
        )
        .await
    }

    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()> {
        LaborProcessRepo::update(
            executor,
            req.id,
            &req.product_code,
            req.process_code.as_deref(),
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref(),
        )
        .await
    }

    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64> {
        LaborProcessRepo::delete(executor, id, product_code).await
    }

    // ========================================================================
    // Excel 导入导出
    // ========================================================================

    async fn import_from_excel(
        &self,
        file_path: &str,
        routing_service: &dyn RoutingService,
    ) -> Result<LaborProcessImportResult> {
        let path = Path::new(file_path);

        // 安全校验：只允许上传目录下的文件
        let upload_dir = std::env::temp_dir().canonicalize().context("无法解析上传目录")?;
        let canonical = path.canonicalize().context("无法解析文件路径")?;
        if !canonical.starts_with(&upload_dir) {
            anyhow::bail!("只允许导入上传目录中的文件");
        }

        let mut excel: Xlsx<_> = open_workbook(&canonical).context("无法打开 Excel 文件")?;
        let range = excel
            .worksheet_range_at(0)
            .ok_or_else(|| anyhow::anyhow!("找不到第一个工作表"))?
            .context("无法读取工作表")?;

        let iter_results = RangeDeserializerBuilder::with_headers(LABOR_PROCESS_EXCEL_COLUMNS)
            .from_range(&range)?;

        // ================================================================
        // 第一遍：解析 + 校验所有行
        // ================================================================
        let mut valid_rows: Vec<ValidLaborProcessRow> = Vec::new();
        let mut results: Vec<LaborProcessImportRowResult> = Vec::new();
        let mut seen_names: std::collections::HashMap<(String, String), i32> = std::collections::HashMap::new();
        let mut failure_count = 0i32;
        let mut row_number = 1i32;

        for result in iter_results {
            row_number += 1;
            let row: ExcelRow = match result {
                Ok(r) => r,
                Err(e) => {
                    failure_count += 1;
                    results.push(row_error(row_number, String::new(), format!("行解析失败: {e}")));
                    continue;
                }
            };

            let product_code = row.product_code.trim().to_string();
            if product_code.is_empty() {
                failure_count += 1;
                results.push(row_error(row_number, String::new(), "产品编码不能为空"));
                continue;
            }

            let process_code = row.process_code.as_ref().and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
            });

            let name = normalize_process_name(&row.name);

            if name.is_empty() {
                failure_count += 1;
                results.push(row_error(row_number, String::new(), "工序名称不能为空"));
                continue;
            }

            let unit_price = match row.unit_price {
                Some(p) if p <= Decimal::ZERO => {
                    failure_count += 1;
                    results.push(row_error(row_number, name.clone(), "单价不能小于等于0"));
                    continue;
                }
                Some(p) => p,
                None => {
                    failure_count += 1;
                    results.push(row_error(row_number, name.clone(), "单价不能为空"));
                    continue;
                }
            };

            let quantity = row.quantity.unwrap_or(Decimal::ONE);
            if quantity < Decimal::ZERO {
                failure_count += 1;
                results.push(row_error(row_number, name.clone(), "数量不能为负数"));
                continue;
            }

            let sort_order = row.sort_order.unwrap_or(row_number);

            // 同一产品内工序名称不能重复
            let name_key = (product_code.clone(), name.clone());
            if let Some(&first_row) = seen_names.get(&name_key) {
                failure_count += 1;
                results.push(row_error(row_number, name.clone(), format!(
                    "产品 {} 内与第 {first_row} 行的工序名称重复", product_code
                )));
                continue;
            }
            seen_names.insert(name_key, row_number);

            valid_rows.push(ValidLaborProcessRow {
                row_number,
                product_code,
                process_code,
                name,
                unit_price,
                quantity,
                sort_order,
                remark: row.remark,
            });
        }

        if failure_count > 0 {
            return Ok(LaborProcessImportResult {
                success_count: 0,
                failure_count,
                results,
                routing_results: Vec::new(),
            });
        }

        // ================================================================
        // 验证所有 process_code 是否存在于工序字典中
        // ================================================================
        let all_unique_process_codes = unique_sorted_process_codes(&valid_rows);

        if !all_unique_process_codes.is_empty() {
            validate_process_codes(&self.pool, &all_unique_process_codes).await?;
        }

        // ================================================================
        // 按产品编码分组（转移所有权）
        // ================================================================
        let mut grouped: std::collections::HashMap<String, Vec<ValidLaborProcessRow>> =
            std::collections::HashMap::new();
        for row in valid_rows {
            grouped.entry(row.product_code.clone()).or_default().push(row);
        }

        // 排序保证确定性顺序
        let mut product_codes: Vec<String> = grouped.keys().cloned().collect();
        product_codes.sort();

        // ================================================================
        // 校验产品编码是否有对应的 BOM
        // ================================================================
        let codes_with_bom = BomRepo::find_product_codes_with_bom(&self.pool, &product_codes).await?;
        let codes_with_bom_set: std::collections::HashSet<&str> =
            codes_with_bom.iter().map(|s| s.as_str()).collect();

        let mut products_to_skip: std::collections::HashSet<String> = std::collections::HashSet::new();
        for pc in &product_codes {
            if !codes_with_bom_set.contains(pc.as_str()) {
                failure_count += 1;
                results.push(LaborProcessImportRowResult {
                    row_number: 0,
                    process_name: pc.clone(),
                    operation: "error".to_string(),
                    error_message: format!("产品 {} 没有对应的 BOM，无法导入人工成本", pc),
                });
                products_to_skip.insert(pc.clone());
            }
        }

        // ================================================================
        // 工艺路线校验：已绑定路线的产品，检查缺失工序和数量为0的备注
        // ================================================================

        for pc in &product_codes {
            let rows_for_product = grouped.get(pc).unwrap();

            match routing_service.get_bom_routing(pc).await {
                Ok(Some((_, routing_name, routing_steps))) => {
                    let mut product_has_error = false;

                    // 检查1：路线中的工序是否在导入中缺失
                    let imported_codes: std::collections::HashSet<&str> = rows_for_product
                        .iter()
                        .filter_map(|r| r.process_code.as_deref())
                        .collect();

                    let missing_steps: Vec<&RoutingStep> = routing_steps
                        .iter()
                        .filter(|s| !imported_codes.contains(s.process_code.as_str()))
                        .collect();

                    if !missing_steps.is_empty() {
                        for step in &missing_steps {
                            failure_count += 1;
                            results.push(LaborProcessImportRowResult {
                                row_number: 0,
                                process_name: format!("{} / {}", pc, step.process_code),
                                operation: "error".to_string(),
                                error_message: format!(
                                    "产品 {} 的路线 '{}' 包含工序 '{}' 但导入中缺失，请添加该工序（数量可为0）并在备注中说明原因",
                                    pc, routing_name, step.process_code
                                ),
                            });
                        }
                        product_has_error = true;
                    }

                    // 检查2：数量为0的工序必须有备注
                    if !product_has_error {
                        for row in rows_for_product {
                            if row.quantity == Decimal::ZERO {
                                let has_remark = row.remark
                                    .as_ref()
                                    .map(|s| !s.trim().is_empty())
                                    .unwrap_or(false);
                                if !has_remark {
                                    failure_count += 1;
                                    results.push(LaborProcessImportRowResult {
                                        row_number: row.row_number,
                                        process_name: row.name.clone(),
                                        operation: "error".to_string(),
                                        error_message: format!(
                                            "产品 {} 的工序 '{}' 数量为 0，需要在备注中说明原因",
                                            pc, row.name
                                        ),
                                    });
                                    product_has_error = true;
                                }
                            }
                        }
                    }

                    if product_has_error {
                        products_to_skip.insert(pc.clone());
                    }
                }
                Ok(None) => {} // 没有路线绑定 — 后续自动创建
                Err(e) => {
                    failure_count += 1;
                    results.push(LaborProcessImportRowResult {
                        row_number: 0,
                        process_name: String::new(),
                        operation: "error".to_string(),
                        error_message: format!("产品 {} 查询路线失败: {}", pc, e),
                    });
                    products_to_skip.insert(pc.clone());
                }
            }
        }

        // 如果所有产品都校验失败，直接返回
        if products_to_skip.len() == product_codes.len() {
            return Ok(LaborProcessImportResult {
                success_count: 0,
                failure_count,
                results,
                routing_results: Vec::new(),
            });
        }

        // ================================================================
        // 事务：按产品依次处理
        // ================================================================
        let mut tx = self.pool.begin().await?;
        let mut routing_results: Vec<PerProductRoutingResult> = Vec::new();
        let mut success_count = 0i32;

        for pc in &product_codes {
            if products_to_skip.contains(pc) {
                continue;
            }

            let rows_for_product = grouped.get(pc).unwrap();
            let product_process_codes = unique_sorted_process_codes(rows_for_product);

            let mut auto_created = false;
            let mut matched_existing = false;
            let mut route_name: Option<String> = None;
            let mut route_id: Option<i64> = None;

            if !product_process_codes.is_empty() {
                let routing_result = auto_route(
                    routing_service,
                    &mut tx,
                    pc,
                    &product_process_codes,
                    rows_for_product,
                )
                .await?;

                auto_created = routing_result.auto_created;
                matched_existing = routing_result.matched_existing;
                route_name = routing_result.name;
                route_id = routing_result.id;
            }

            LaborProcessRepo::delete_by_product_code(&mut tx, pc).await?;
            LaborProcessRepo::batch_insert(&mut tx, pc, rows_for_product).await?;

            routing_results.push(PerProductRoutingResult {
                product_code: pc.clone(),
                auto_created_routing: auto_created,
                matched_existing_routing: matched_existing,
                routing_name: route_name,
                routing_id: route_id,
            });

            success_count += rows_for_product.len() as i32;
            for row in rows_for_product {
                results.push(LaborProcessImportRowResult {
                    row_number: row.row_number,
                    process_name: row.name.clone(),
                    operation: "created".to_string(),
                    error_message: String::new(),
                });
            }
        }

        tx.commit().await?;

        Ok(LaborProcessImportResult {
            success_count,
            failure_count,
            results,
            routing_results,
        })
    }

    async fn export_to_bytes(&self, product_code: &str) -> Result<Vec<u8>> {
        let processes = LaborProcessRepo::list_all_by_product_code(&self.pool, product_code).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        for (col, header) in LABOR_PROCESS_EXCEL_COLUMNS.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, p) in processes.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &p.product_code)?;
            worksheet.write_string(row_num, 1, p.process_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 2, &p.name)?;
            worksheet.write_number(row_num, 3, p.unit_price.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 4, p.quantity.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 5, p.sort_order as f64)?;
            worksheet.write_string(row_num, 6, p.remark.as_deref().unwrap_or(""))?;
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }

    async fn export_boms_without_labor_cost(&self) -> Result<Vec<u8>> {
        let boms = LaborProcessRepo::find_boms_without_labor_cost(&self.pool).await?;

        let headers = ["BOM名称", "产品编码", "产品名称", "创建时间"];
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, b) in boms.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &b.bom_name)?;
            worksheet.write_string(row_num, 1, b.product_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 2, b.product_name.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 3, &b.created_at.format("%Y-%m-%d %H:%M").to_string())?;
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

fn row_error(row_number: i32, process_name: String, msg: impl Into<String>) -> LaborProcessImportRowResult {
    LaborProcessImportRowResult {
        row_number,
        process_name,
        operation: "error".to_string(),
        error_message: msg.into(),
    }
}

/// 从行集合中提取去重排序后的工序编码
fn unique_sorted_process_codes(rows: &[ValidLaborProcessRow]) -> Vec<String> {
    let mut sorted: Vec<String> = rows
        .iter()
        .filter_map(|r| r.process_code.as_ref())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    sorted.sort();
    sorted
}

/// 验证所有 process_code 是否存在于工序字典中
async fn validate_process_codes(
    pool: &PgPool,
    process_codes: &[String],
) -> Result<()> {
    let valid_codes = LaborProcessDictRepo::find_existing_codes(pool, process_codes).await?;

    let unknown: Vec<&str> = process_codes
        .iter()
        .filter(|code| !valid_codes.contains(*code))
        .map(|s| s.as_str())
        .collect();

    if !unknown.is_empty() {
        return Err(common::error::ServiceError::BusinessValidation {
            message: format!("以下工序编码不存在于工序字典中: {}", unknown.join(", ")),
        }
        .into());
    }

    Ok(())
}

/// 自动路线匹配/创建的结果
struct AutoRouteResult {
    auto_created: bool,
    matched_existing: bool,
    name: Option<String>,
    id: Option<i64>,
}

/// 已绑定路线的产品在校验阶段已通过 routing validation，这里不再重复检查
async fn auto_route(
    routing_service: &dyn RoutingService,
    executor: Executor<'_>,
    product_code: &str,
    unique_process_codes: &[String],
    valid_rows: &[ValidLaborProcessRow],
) -> Result<AutoRouteResult> {
    // 检查产品是否已有路线绑定
    let existing = routing_service.get_bom_routing_tx(product_code, executor).await?;

    if let Some((existing_id, existing_name, _steps)) = existing {
        return Ok(AutoRouteResult {
            auto_created: false,
            matched_existing: true,
            name: Some(existing_name),
            id: Some(existing_id),
        });
    }

    // 产品没有路线绑定，尝试匹配已有路线
    let matched = routing_service.find_matching_routing_tx(unique_process_codes, executor).await?;

    if let Some(matched_id) = matched {
        // 尝试绑定匹配到的路线 — 仅在路线已被删除时回退，其他错误上抛
        let bind_result = async {
            routing_service
                .set_bom_routing(product_code, matched_id, executor)
                .await?;
            let detail = routing_service.get_detail_tx(matched_id, executor).await?;
            Ok::<_, anyhow::Error>((detail.0.name, matched_id))
        }
        .await;

        match bind_result {
            Ok((name, id)) => {
                return Ok(AutoRouteResult {
                    auto_created: false,
                    matched_existing: true,
                    name: Some(name),
                    id: Some(id),
                });
            }
            Err(e) => {
                // 仅 NotFound（路线已被删除）可以回退创建新路线，其他错误上抛
                let msg = e.to_string();
                if !msg.contains("not found") && !msg.contains("未找到") {
                    return Err(e);
                }
            }
        }
    }

    // 创建新路线
    let now = Utc::now();
    let date_str = now.format("%Y%m%d").to_string();
    let routing_name = format!("Auto-{}-{}", product_code, date_str);

    // 从 valid_rows 中提取工序步骤（按 Excel 出现顺序）
    let mut steps: Vec<RoutingStepInput> = Vec::new();
    let mut seen_codes: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut step_order = 1i32;

    for row in valid_rows {
        if let Some(code) = &row.process_code
            && !seen_codes.contains(code)
        {
            seen_codes.insert(code.clone());
            steps.push(RoutingStepInput {
                process_code: code.clone(),
                step_order,
                is_required: true,
                remark: row.remark.clone(),
            });
            step_order += 1;
        }
    }

    let create_req = CreateRoutingReq {
        name: routing_name.clone(),
        description: Some(format!("导入工序时自动创建 ({})", date_str)),
        steps,
    };

    // 创建新路线（在同一个 executor/事务内）
    let new_routing_id = routing_service
        .create(create_req, executor)
        .await?;

    // 绑定产品到新路线
    routing_service
        .set_bom_routing(product_code, new_routing_id, executor)
        .await?;

    Ok(AutoRouteResult {
        auto_created: true,
        matched_existing: false,
        name: Some(routing_name),
        id: Some(new_routing_id),
    })
}

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "产品编码")]
    product_code: String,
    #[serde(rename = "工序编码")]
    process_code: Option<String>,
    #[serde(rename = "工序名称")]
    name: String,
    #[serde(rename = "单价", deserialize_with = "deserialize_decimal_opt")]
    unit_price: Option<Decimal>,
    #[serde(rename = "数量", deserialize_with = "deserialize_decimal_opt")]
    quantity: Option<Decimal>,
    #[serde(rename = "排序", deserialize_with = "deserialize_int_opt")]
    sort_order: Option<i32>,
    #[serde(rename = "备注")]
    remark: Option<String>,
}

fn deserialize_decimal_opt<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Decimal::from_str_exact(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn deserialize_int_opt<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s.parse()
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn normalize_process_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '\u{3000}' => result.push(' '),
            '\u{FF08}' => result.push('('),
            '\u{FF09}' => result.push(')'),
            '\u{FF1A}' => result.push(':'),
            '\u{FF1B}' => result.push(';'),
            '\u{FF0C}' => result.push(','),
            '\u{3002}' => result.push('.'),
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => {}
            other => result.push(other),
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_fullwidth_spaces() {
        assert_eq!(normalize_process_name("工序\u{3000}名称"), "工序 名称");
    }

    #[test]
    fn test_normalize_fullwidth_parens() {
        assert_eq!(normalize_process_name("组装（人工）"), "组装(人工)");
    }

    #[test]
    fn test_normalize_fullwidth_punctuation() {
        assert_eq!(normalize_process_name("名称：工序；备注，说明。"), "名称:工序;备注,说明.");
    }

    #[test]
    fn test_normalize_zero_width_chars() {
        assert_eq!(normalize_process_name("A\u{200B}B\u{FEFF}C\u{200D}D"), "ABCD");
    }

    #[test]
    fn test_normalize_trim() {
        assert_eq!(normalize_process_name("  工序  "), "工序");
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_process_name(""), "");
        assert_eq!(normalize_process_name("   "), "");
    }

    #[test]
    fn test_normalize_passthrough() {
        assert_eq!(normalize_process_name("焊接"), "焊接");
    }
}
