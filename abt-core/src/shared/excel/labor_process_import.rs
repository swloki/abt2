//! 工序 Excel 导入实现
//!
//! 适配 abt_v2 的 bom_labor_processes、routings、bom_routings 表。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use calamine::RangeDeserializerBuilder;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use super::helpers::{ProgressTracker, deserialize_optional_decimal, import_range_from_source};
use super::types::ImportSource;
use crate::master_data::bom::repo::BomRepo;
use crate::master_data::bom_labor_process::repo::{BomLaborProcessRepo, LaborProcessRow};
use crate::master_data::labor_process_dict::repo::LaborProcessDictRepo;
use crate::master_data::routing::repo::RoutingRepo;
use crate::master_data::routing::model::RoutingStep;

const LABOR_PROCESS_EXCEL_COLUMNS: [&str; 7] = [
    "产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注",
];

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
        Some(s) => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

struct ValidRow {
    row_number: i32,
    product_code: String,
    process_code: Option<String>,
    name: String,
    unit_price: Decimal,
    quantity: Decimal,
    sort_order: i32,
    remark: Option<String>,
}

/// 工序导入结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaborProcessImportResult {
    pub success_count: i32,
    pub failure_count: i32,
    pub results: Vec<RowResult>,
    pub routing_results: Vec<RoutingMatchResult>,
}

impl LaborProcessImportResult {
    pub fn failed(count: i32, results: Vec<RowResult>) -> Self {
        Self {
            success_count: 0,
            failure_count: count,
            results,
            routing_results: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RowResult {
    pub row_number: i32,
    pub process_name: String,
    pub operation: String,
    pub error_message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingMatchResult {
    pub product_code: String,
    pub matched_existing_routing: bool,
    pub routing_name: Option<String>,
    pub routing_id: Option<i64>,
}

fn row_error(row_number: i32, process_name: String, msg: impl Into<String>) -> RowResult {
    RowResult {
        row_number,
        process_name,
        operation: "error".to_string(),
        error_message: msg.into(),
    }
}

pub struct LaborProcessImporter {
    pool: PgPool,
    tracker: Arc<ProgressTracker>,
}

impl LaborProcessImporter {
    pub fn new(pool: PgPool, tracker: Arc<ProgressTracker>) -> Self {
        Self { pool, tracker }
    }

    pub async fn import(&self, source: ImportSource) -> Result<LaborProcessImportResult> {
        let range = import_range_from_source(source)?;

        let iter_results = RangeDeserializerBuilder::with_headers(&LABOR_PROCESS_EXCEL_COLUMNS)
            .from_range(&range)?;

        let total = range.rows().count().saturating_sub(1);
        self.tracker.set_total(total);

        let mut valid_rows: Vec<ValidRow> = Vec::new();
        let mut results: Vec<RowResult> = Vec::new();
        let mut seen_names: HashMap<(String, String), i32> = HashMap::new();
        let mut failure_count = 0i32;
        let mut row_number = 1i32;

        for res in iter_results {
            row_number += 1;
            let row: ExcelRow = match res {
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

            valid_rows.push(ValidRow {
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

        // 验证 process_code 存在于工序字典
        let all_process_codes = unique_sorted_process_codes(&valid_rows);
        if !all_process_codes.is_empty() {
            let mut conn = self.pool.acquire().await?;
            let valid_codes = LaborProcessDictRepo {}
                .find_existing_codes(&mut conn, &all_process_codes)
                .await?;
            let unknown: Vec<&str> = all_process_codes
                .iter()
                .filter(|code| !valid_codes.contains(code))
                .map(|s| s.as_str())
                .collect();
            if !unknown.is_empty() {
                failure_count += 1;
                results.push(row_error(0, String::new(), format!(
                    "以下工序编码不存在于工序字典中: {}", unknown.join(", ")
                )));
                return Ok(LaborProcessImportResult::failed(failure_count, results));
            }
        }

        // 按产品编码分组
        let mut grouped: HashMap<String, Vec<&ValidRow>> = HashMap::new();
        for row in &valid_rows {
            grouped.entry(row.product_code.clone()).or_default().push(row);
        }

        let mut product_codes: Vec<String> = grouped.keys().cloned().collect();
        product_codes.sort();

        // 验证产品编码有对应 BOM
        let mut conn = self.pool.acquire().await?;
        let codes_with_bom = BomRepo {}.find_product_codes_with_bom(&mut conn, &product_codes).await?;
        let codes_with_bom_set: HashSet<&str> = codes_with_bom.iter().map(|s| s.as_str()).collect();

        let mut products_to_skip: HashSet<String> = HashSet::new();
        for pc in &product_codes {
            if !codes_with_bom_set.contains(pc.as_str()) {
                failure_count += 1;
                results.push(row_error(0, pc.clone(), format!("产品 {} 没有对应的 BOM，无法导入人工成本", pc)));
                products_to_skip.insert(pc.clone());
            }
        }

        // 批量预加载路线数据
        let routing_repo = RoutingRepo {};
        let mut bom_routing_map: HashMap<String, (i64, Option<i64>)> = HashMap::new();
        for pc in &product_codes {
            if let Some(br) = routing_repo.get_bom_routing(&mut conn, pc).await? {
                bom_routing_map.insert(pc.clone(), (br.routing_id, None));
            }
        }

        // 验证路线完整性
        for pc in &product_codes {
            let Some(rows_for_product) = grouped.get(pc) else { continue };

            if let Some((routing_id, _)) = bom_routing_map.get(pc) {
                let routing_steps = routing_repo.find_steps(&mut conn, *routing_id).await?;

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
                            "产品 {} 的路线包含工序 '{}' 但导入中缺失", pc, step.process_code
                        )));
                    }
                    products_to_skip.insert(pc.clone());
                }
            } else {
                // 无绑定路线，尝试匹配
                let codes = unique_sorted_process_codes_from_refs(rows_for_product);
                if !codes.is_empty() {
                    let matched = routing_repo.find_matching_by_process_codes(&mut conn, &codes).await?;
                    if matched.is_none() {
                        failure_count += 1;
                        results.push(row_error(0, pc.clone(), format!(
                            "未找到匹配的工艺路线（工序编码: {}）", codes.join(", ")
                        )));
                        products_to_skip.insert(pc.clone());
                    }
                }
            }
        }

        if products_to_skip.len() == product_codes.len() {
            return Ok(LaborProcessImportResult::failed(failure_count, results));
        }

        // 分批事务：每 500 个产品一个事务
        const BATCH_SIZE: usize = 500;
        let processable: Vec<&String> = product_codes
            .iter()
            .filter(|pc| !products_to_skip.contains(*pc))
            .collect();

        let mut routing_results: Vec<RoutingMatchResult> = Vec::new();
        let mut success_count = 0i32;

        for chunk in processable.chunks(BATCH_SIZE) {
            let mut tx = self.pool.begin().await?;

            for &pc in chunk {
                let Some(rows_for_product) = grouped.get(pc) else { continue };

                sqlx::query("SAVEPOINT product_sp")
                    .execute(&mut *tx)
                    .await?;

                let mut product_failed = false;
                let mut route_name: Option<String> = None;
                let mut route_id: Option<i64> = None;

                let product_process_codes = unique_sorted_process_codes_from_refs(rows_for_product);

                if !product_process_codes.is_empty() {
                    match find_and_bind_routing(&routing_repo, &mut tx, pc, &product_process_codes).await {
                        Ok(r) => {
                            route_name = r.name;
                            route_id = r.id;
                        }
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("查找或绑定路线失败: {}", e)));
                            product_failed = true;
                        }
                    }
                }

                if !product_failed {
                    let delete_result = BomLaborProcessRepo::delete_by_product_code(&mut tx, pc).await;
                    match delete_result {
                        Ok(_) => {}
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("删除现有工序失败: {}", e)));
                            product_failed = true;
                        }
                    }
                }

                if !product_failed {
                    let insert_rows: Vec<LaborProcessRow> = rows_for_product
                        .iter()
                        .map(|r| {
                            let dict_id: i64 = 0; // process_code → dict_id 查找在 import 层面简化
                            (
                                pc.clone(),
                                dict_id,
                                r.process_code.clone().unwrap_or_default(),
                                r.name.clone(),
                                r.unit_price,
                                r.quantity,
                                r.sort_order,
                                r.remark.clone(),
                            )
                        })
                        .collect();

                    match BomLaborProcessRepo::batch_insert(&mut tx, &insert_rows).await {
                        Ok(_) => {}
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, pc.clone(), format!("插入工序失败: {}", e)));
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

                    routing_results.push(RoutingMatchResult {
                        product_code: pc.clone(),
                        matched_existing_routing: route_name.is_some(),
                        routing_name: route_name,
                        routing_id: route_id,
                    });

                    success_count += rows_for_product.len() as i32;
                    for row in rows_for_product {
                        results.push(RowResult {
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

struct AutoRouteResult {
    name: Option<String>,
    id: Option<i64>,
}

async fn find_and_bind_routing(
    routing_repo: &RoutingRepo,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    product_code: &str,
    process_codes: &[String],
) -> Result<AutoRouteResult> {
    let existing = routing_repo.get_bom_routing(tx, product_code).await?;
    if let Some(br) = existing {
        let routing = routing_repo.find_by_id(tx, br.routing_id).await?;
        if let Some(r) = routing {
            return Ok(AutoRouteResult {
                name: Some(r.name),
                id: Some(r.id),
            });
        }
    }

    let matched = routing_repo.find_matching_by_process_codes(tx, process_codes).await?;
    if let Some(matched_id) = matched {
        routing_repo.set_bom_routing(tx, product_code, matched_id, 0).await?;
        let routing = routing_repo.find_by_id(tx, matched_id).await?;
        if let Some(r) = routing {
            return Ok(AutoRouteResult {
                name: Some(r.name),
                id: Some(r.id),
            });
        }
    }

    Err(anyhow::anyhow!(
        "未找到匹配的工艺路线（工序编码: {}）",
        process_codes.join(", ")
    ))
}

fn unique_sorted_process_codes(rows: &[ValidRow]) -> Vec<String> {
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

fn unique_sorted_process_codes_from_refs(rows: &[&ValidRow]) -> Vec<String> {
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
    fn test_normalize_empty() {
        assert_eq!(normalize_process_name(""), "");
        assert_eq!(normalize_process_name("   "), "");
    }
}
