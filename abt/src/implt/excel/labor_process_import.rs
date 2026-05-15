//! 工序 Excel 导入实现

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use calamine::RangeDeserializerBuilder;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use crate::implt::excel::{ProgressTracker, deserialize_optional_decimal, import_range_from_source};
use crate::models::{
    LaborProcessImportResult, LaborProcessImportRowResult, PerProductRoutingResult,
    RoutingStep, ValidLaborProcessRow, LABOR_PROCESS_EXCEL_COLUMNS,
};
use crate::repositories::{BomRepo, Executor, LaborProcessDictRepo, LaborProcessRepo, RoutingRepo};
use crate::service::{ImportSource, RoutingService};

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "产品编码")]
    product_code: String,
    #[serde(rename = "工序编码")]
    process_code: Option<String>,
    #[serde(rename = "工序名称")]
    name: String,
    #[serde(rename = "单价", deserialize_with = "deserialize_optional_decimal")]
    unit_price: Option<Decimal>,
    #[serde(rename = "数量", deserialize_with = "deserialize_optional_decimal")]
    quantity: Option<Decimal>,
    #[serde(rename = "排序", deserialize_with = "deserialize_int_opt")]
    sort_order: Option<i32>,
    #[serde(rename = "备注")]
    remark: Option<String>,
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

pub struct LaborProcessImporter {
    pool: PgPool,
    tracker: Arc<ProgressTracker>,
    routing_service: Arc<dyn RoutingService>,
}

impl LaborProcessImporter {
    pub fn new(
        pool: PgPool,
        tracker: Arc<ProgressTracker>,
        routing_service: Arc<dyn RoutingService>,
    ) -> Self {
        Self { pool, tracker, routing_service }
    }

    /// 从 Excel 导入工序数据，保留结构化行结果和路线信息
    ///
    /// 注意：返回 `LaborProcessImportResult` 而非 `ImportResult`，
    /// 因为工序导入需要保留结构化的行结果和工艺路线信息供前端展示。
    pub async fn import(&self, source: ImportSource) -> Result<LaborProcessImportResult> {
        let range = import_range_from_source(source)?;

        let iter_results = RangeDeserializerBuilder::with_headers(&LABOR_PROCESS_EXCEL_COLUMNS)
            .from_range(&range)?;

        let total = range.rows().count().saturating_sub(1);
        self.tracker.set_total(total);

        // 第一遍：解析 + 校验所有行
        let mut valid_rows: Vec<ValidLaborProcessRow> = Vec::new();
        let mut results: Vec<LaborProcessImportRowResult> = Vec::new();
        let mut seen_names: HashMap<(String, String), i32> = HashMap::new();
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

        if valid_rows.is_empty() {
            return Ok(LaborProcessImportResult::failed(failure_count, results));
        }

        // 验证所有 process_code 是否存在于工序字典中
        let all_unique_process_codes = unique_sorted_process_codes(&valid_rows);

        if !all_unique_process_codes.is_empty()
            && let Err(e) = validate_process_codes(&self.pool, &all_unique_process_codes).await
        {
            failure_count += 1;
            results.push(row_error(0, String::new(), e.to_string()));
            return Ok(LaborProcessImportResult::failed(failure_count, results));
        }

        // 按产品编码分组
        let mut grouped: HashMap<String, Vec<ValidLaborProcessRow>> = HashMap::new();
        for row in valid_rows {
            grouped.entry(row.product_code.clone()).or_default().push(row);
        }

        let mut product_codes: Vec<String> = grouped.keys().cloned().collect();
        product_codes.sort();

        // 校验产品编码是否有对应的 BOM
        let codes_with_bom = BomRepo::find_product_codes_with_bom(&self.pool, &product_codes).await?;
        let codes_with_bom_set: HashSet<&str> =
            codes_with_bom.iter().map(|s| s.as_str()).collect();

        let mut products_to_skip: HashSet<String> = HashSet::new();
        for pc in &product_codes {
            if !codes_with_bom_set.contains(pc.as_str()) {
                failure_count += 1;
                results.push(row_error(0, pc.clone(), format!("产品 {} 没有对应的 BOM，无法导入人工成本", pc)));
                products_to_skip.insert(pc.clone());
            }
        }

        // 批量预加载路线数据（消除 N+1 查询）
        let bom_routing_map = RoutingRepo::find_bom_routing_batch(&self.pool, &product_codes).await?;
        let routing_ids: Vec<i64> = bom_routing_map.values().map(|b| b.routing_id).collect();
        let routing_map = RoutingRepo::find_routing_by_ids(&self.pool, &routing_ids).await?;
        let steps_map = RoutingRepo::find_steps_by_routing_ids_batch(&self.pool, &routing_ids).await?;

        // 工艺路线校验（使用预加载的批量数据）
        for pc in &product_codes {
            let Some(rows_for_product) = grouped.get(pc) else {
                continue;
            };

            if let Some(binding) = bom_routing_map.get(pc) {
                let routing = match routing_map.get(&binding.routing_id) {
                    Some(r) => r,
                    None => {
                        failure_count += 1;
                        results.push(row_error(0, String::new(), format!("产品 {} 绑定的路线已被删除", pc)));
                        products_to_skip.insert(pc.clone());
                        continue;
                    }
                };
                let routing_steps = steps_map.get(&binding.routing_id).cloned().unwrap_or_default();

                let mut product_has_error = false;

                let imported_codes: HashSet<&str> = rows_for_product
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
                        results.push(row_error(0, format!("{} / {}", pc, step.process_code), format!(
                            "产品 {} 的路线 '{}' 包含工序 '{}' 但导入中缺失，请添加该工序（数量可为0）并在备注中说明原因",
                            pc, routing.name, step.process_code
                        )));
                    }
                    product_has_error = true;
                }

                if !product_has_error {
                    for row in rows_for_product {
                        if row.quantity == Decimal::ZERO {
                            let has_remark = row.remark
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false);
                            if !has_remark {
                                failure_count += 1;
                                results.push(row_error(row.row_number, row.name.clone(), format!(
                                    "产品 {} 的工序 '{}' 数量为 0，需要在备注中说明原因",
                                    pc, row.name
                                )));
                                product_has_error = true;
                            }
                        }
                    }
                }

                if product_has_error {
                    products_to_skip.insert(pc.clone());
                }
            } else {
                // 无绑定路线，尝试匹配
                let codes = unique_sorted_process_codes(rows_for_product);
                if !codes.is_empty() {
                    let matched = match self.routing_service.find_matching_routing(&codes).await {
                        Ok(m) => m,
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, String::new(), format!("产品 {} 查询匹配路线失败: {}", pc, e)));
                            products_to_skip.insert(pc.clone());
                            continue;
                        }
                    };
                    if matched.is_none() {
                        failure_count += 1;
                        results.push(row_error(0, pc.clone(), format!(
                            "未找到匹配的工艺路线（工序编码: {}），请先在工艺路线管理中创建对应路线后再导入",
                            codes.join(", ")
                        )));
                        products_to_skip.insert(pc.clone());
                    }
                }
            }
        }

        if products_to_skip.len() == product_codes.len() {
            return Ok(LaborProcessImportResult::failed(failure_count, results));
        }

        // 分批事务：每 500 个产品一个事务，避免单事务过大超时
        const BATCH_SIZE: usize = 500;
        let processable: Vec<&String> = product_codes
            .iter()
            .filter(|pc| !products_to_skip.contains(*pc))
            .collect();

        let mut routing_results: Vec<PerProductRoutingResult> = Vec::new();
        let mut success_count = 0i32;

        for chunk in processable.chunks(BATCH_SIZE) {
            let mut tx = self.pool.begin().await?;

            for &pc in chunk {
                let Some(rows_for_product) = grouped.get(pc) else {
                continue;
            };
                let product_process_codes = unique_sorted_process_codes(rows_for_product);

                let mut route_name: Option<String> = None;
                let mut route_id: Option<i64> = None;
                let mut product_failed = false;

                sqlx::query("SAVEPOINT product_sp")
                    .execute(&mut *tx)
                    .await?;

                if !product_process_codes.is_empty() {
                    let routing_result = match find_and_bind_routing(
                        self.routing_service.as_ref(),
                        &mut tx,
                        pc,
                        &product_process_codes,
                    )
                    .await {
                        Ok(r) => r,
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("产品 {} 查找或绑定路线失败: {}", pc, e)));
                            product_failed = true;
                            AutoRouteResult { name: None, id: None }
                        }
                    };

                    if !product_failed {
                        route_name = routing_result.name;
                        route_id = routing_result.id;
                    }
                }

                if !product_failed {
                    match LaborProcessRepo::delete_by_product_code(&mut tx, pc).await {
                        Ok(_) => {},
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("产品 {} 删除现有工序失败: {}", pc, e)));
                            product_failed = true;
                        }
                    }
                }

                if !product_failed {
                    match LaborProcessRepo::batch_insert(&mut tx, pc, rows_for_product).await {
                        Ok(_) => {},
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("产品 {} 插入工序失败: {}", pc, e)));
                            product_failed = true;
                        }
                    }
                }

                if product_failed {
                    sqlx::query("ROLLBACK TO SAVEPOINT product_sp")
                        .execute(&mut *tx)
                        .await?;
                } else {
                    sqlx::query("RELEASE SAVEPOINT product_sp")
                        .execute(&mut *tx)
                        .await?;

                    self.tracker.tick();

                    routing_results.push(PerProductRoutingResult {
                        product_code: pc.clone(),
                        matched_existing_routing: route_name.is_some(),
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
            }

            tx.commit().await?;
        }

        Ok(LaborProcessImportResult {
            success_count,
            failure_count,
            results,
            routing_results,
        })
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

fn unique_sorted_process_codes(rows: &[ValidLaborProcessRow]) -> Vec<String> {
    let mut sorted: Vec<String> = rows
        .iter()
        .filter_map(|r| r.process_code.as_ref())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    sorted.sort();
    sorted
}

async fn validate_process_codes(pool: &PgPool, process_codes: &[String]) -> Result<()> {
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

struct AutoRouteResult {
    name: Option<String>,
    id: Option<i64>,
}

async fn find_and_bind_routing(
    routing_service: &dyn RoutingService,
    executor: Executor<'_>,
    product_code: &str,
    unique_process_codes: &[String],
) -> Result<AutoRouteResult> {
    let existing = routing_service.get_bom_routing_tx(product_code, executor).await?;

    if let Some((existing_id, existing_name, _steps)) = existing {
        return Ok(AutoRouteResult {
            name: Some(existing_name),
            id: Some(existing_id),
        });
    }

    let matched = routing_service.find_matching_routing_tx(unique_process_codes, executor).await?;

    if let Some(matched_id) = matched {
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
                    name: Some(name),
                    id: Some(id),
                });
            }
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("not found") && !msg.contains("未找到") {
                    return Err(e);
                }
            }
        }
    }

    Err(common::error::ServiceError::BusinessValidation {
        message: format!(
            "未找到匹配的工艺路线（工序编码: {}），请先在工艺路线管理中创建对应路线后再导入",
            unique_process_codes.join(", ")
        ),
    }.into())
}

pub fn normalize_process_name(name: &str) -> String {
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
