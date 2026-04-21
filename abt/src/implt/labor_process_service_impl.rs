//! 劳务工序服务实现

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use calamine::{RangeDeserializerBuilder, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use serde::Deserialize;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, LaborProcessRepo};
use crate::service::LaborProcessService;

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
    // 工序 CRUD
    // ========================================================================

    async fn list_processes(&self, query: LaborProcessQuery) -> Result<(Vec<LaborProcess>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessRepo::list(&self.pool, page, page_size, kw).await?;
        let total = LaborProcessRepo::count(&self.pool, kw).await?;
        Ok((items, total))
    }

    async fn create_process(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64> {
        LaborProcessRepo::insert(executor, &req.name, req.unit_price, req.remark.as_deref()).await
    }

    async fn update_process(
        &self,
        req: UpdateLaborProcessReq,
        executor: Executor<'_>,
    ) -> Result<Option<PriceChangeImpact>> {
        let old_price = LaborProcessRepo::get_unit_price(&self.pool, req.id).await?;
        let price_changed = old_price.is_some_and(|p| p != req.unit_price);

        LaborProcessRepo::update(executor, req.id, &req.name, req.unit_price, req.remark.as_deref()).await?;

        if price_changed {
            let (affected_bom_count, affected_item_count) =
                LaborProcessRepo::price_change_impact(&self.pool, req.id).await?;
            Ok(Some(PriceChangeImpact {
                affected_bom_count,
                affected_item_count,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_process(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        let referenced = LaborProcessRepo::is_process_referenced(&mut *executor, id).await?;
        if referenced {
            anyhow::bail!("工序被引用，无法删除");
        }
        LaborProcessRepo::delete(executor, id).await
    }

    // ========================================================================
    // 工序组 CRUD
    // ========================================================================

    async fn list_groups(&self, query: LaborProcessGroupQuery) -> Result<(Vec<LaborProcessGroupWithMembers>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();

        let groups = LaborProcessRepo::list_groups(&self.pool, page, page_size, kw).await?;
        let total = LaborProcessRepo::count_groups(&self.pool, kw).await?;

        let group_ids: Vec<i64> = groups.iter().map(|g| g.id).collect();
        let all_members = LaborProcessRepo::list_group_members_batch(&self.pool, &group_ids).await?;

        let mut members_map: HashMap<i64, Vec<LaborProcessGroupMember>> = HashMap::new();
        for member in all_members {
            members_map.entry(member.group_id).or_default().push(member);
        }

        let result = groups
            .into_iter()
            .map(|group| {
                let members = members_map.remove(&group.id).unwrap_or_default();
                LaborProcessGroupWithMembers { group, members }
            })
            .collect();

        Ok((result, total))
    }

    async fn create_group(&self, req: CreateLaborProcessGroupReq, executor: Executor<'_>) -> Result<i64> {
        if req.members.is_empty() {
            anyhow::bail!("工序组至少需要一个成员");
        }

        let group_id = LaborProcessRepo::insert_group(
            executor,
            &req.name,
            req.remark.as_deref(),
        )
        .await?;

        let members = to_member_tuples(&req.members);
        LaborProcessRepo::set_group_members(executor, group_id, &members).await?;

        Ok(group_id)
    }

    async fn update_group(&self, req: UpdateLaborProcessGroupReq, executor: Executor<'_>) -> Result<()> {
        LaborProcessRepo::update_group(executor, req.id, &req.name, req.remark.as_deref()).await?;

        let members = to_member_tuples(&req.members);
        LaborProcessRepo::set_group_members(executor, req.id, &members).await?;

        Ok(())
    }

    async fn delete_group(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        let referenced = LaborProcessRepo::is_group_referenced_by_bom(&mut *executor, id).await?;
        if referenced {
            anyhow::bail!("工序组被 BOM 引用，无法删除");
        }
        LaborProcessRepo::delete_group(executor, id).await
    }

    // ========================================================================
    // BOM 劳务成本
    // ========================================================================

    async fn set_bom_labor_cost(&self, req: SetBomLaborCostReq, executor: Executor<'_>) -> Result<()> {
        let process_ids: Vec<i64> = req.items.iter().map(|i| i.process_id).collect();
        let prices = LaborProcessRepo::lock_and_get_unit_prices(executor, &process_ids).await?;

        for item in &req.items {
            if !prices.contains_key(&item.process_id) {
                anyhow::bail!("工序 {} 不存在", item.process_id);
            }
        }

        let cost_items: Vec<(i64, Decimal, Option<Decimal>, Option<&str>)> = req
            .items
            .iter()
            .map(|item| {
                let snapshot = prices.get(&item.process_id).copied();
                (item.process_id, item.quantity, snapshot, item.remark.as_deref())
            })
            .collect();

        LaborProcessRepo::clear_bom_labor_cost(executor, req.bom_id).await?;
        LaborProcessRepo::batch_insert_bom_labor_cost(executor, req.bom_id, &cost_items).await?;
        LaborProcessRepo::set_bom_process_group(executor, req.bom_id, req.process_group_id).await?;

        Ok(())
    }

    async fn get_bom_labor_cost(&self, bom_id: i64) -> Result<Option<(LaborProcessGroupWithMembers, Vec<BomLaborCostItem>)>> {
        let group_with_members = LaborProcessRepo::get_bom_group_with_members(&self.pool, bom_id).await?;
        if group_with_members.is_none() {
            return Ok(None);
        }

        let items = LaborProcessRepo::get_bom_labor_cost(&self.pool, bom_id).await?;

        Ok(Some((group_with_members.unwrap(), items)))
    }

    // ========================================================================
    // Excel 导入导出
    // ========================================================================

    async fn import_processes_from_excel(
        &self,
        file_path: &str,
    ) -> Result<LaborProcessImportResult> {
        let path = Path::new(file_path);

        // 安全校验：只允许上传目录下的文件，防止路径遍历
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

        let mut valid_rows: Vec<(String, Decimal, Option<String>)> = Vec::new();
        let mut results: Vec<LaborProcessImportRowResult> = Vec::new();
        let mut seen_names: HashMap<String, i32> = HashMap::new();
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

            let name = normalize_process_name(&row.name);

            if name.is_empty() {
                failure_count += 1;
                results.push(row_error(row_number, String::new(), "工序名称不能为空"));
                continue;
            }

            let unit_price = match row.unit_price {
                Some(p) if p < Decimal::ZERO => {
                    failure_count += 1;
                    results.push(row_error(row_number, name, "单价不能为负数"));
                    continue;
                }
                Some(p) => p,
                None => {
                    failure_count += 1;
                    results.push(row_error(row_number, name, "单价不能为空"));
                    continue;
                }
            };

            if let Some(&first_row) = seen_names.get(&name) {
                failure_count += 1;
                results.push(row_error(row_number, name, format!("与第 {first_row} 行的工序名称重复")));
                continue;
            }
            seen_names.insert(name.clone(), row_number);

            valid_rows.push((name, unit_price, row.remark));
        }

        if failure_count > 0 {
            return Ok(LaborProcessImportResult {
                success_count: 0,
                failure_count,
                skip_count: 0,
                results,
                affected_bom_count: 0,
            });
        }

        let names: Vec<String> = valid_rows.iter().map(|(n, _, _)| n.clone()).collect();
        let existing = LaborProcessRepo::find_by_names(&self.pool, &names).await?;

        // 标准化数据库名称作为 key，确保全角/半角差异不影响匹配
        let existing_map: HashMap<String, LaborProcess> =
            existing.into_iter().map(|p| (normalize_process_name(&p.name), p)).collect();

        let mut to_upsert: Vec<(String, Decimal, Option<String>)> = Vec::new();
        let mut updated_price_process_ids: Vec<i64> = Vec::new();

        for (name, unit_price, remark) in &valid_rows {
            if let Some(existing_p) = existing_map.get(name) {
                let price_changed = existing_p.unit_price != *unit_price;
                let remark_changed = existing_p.remark != *remark;
                if price_changed || remark_changed {
                    // 使用数据库中的原始名称，确保 ON CONFLICT (name) 能正确匹配
                    to_upsert.push((existing_p.name.clone(), *unit_price, remark.clone()));
                    if price_changed {
                        updated_price_process_ids.push(existing_p.id);
                    }
                }
            } else {
                to_upsert.push((name.clone(), *unit_price, remark.clone()));
            }
        }

        let skip_count = (valid_rows.len() - to_upsert.len()) as i32;

        let affected_bom_count = LaborProcessRepo::count_affected_boms_batch(&self.pool, &updated_price_process_ids).await?;

        if !to_upsert.is_empty() {
            let mut tx = self.pool.begin().await?;
            LaborProcessRepo::batch_upsert(&mut tx, &to_upsert).await?;
            tx.commit().await?;
        }

        // 构建已 upsert 的标准化名称集合，用于结果报告
        let upserted_normalized: HashSet<String> = to_upsert.iter()
            .map(|(db_name, _, _)| normalize_process_name(db_name))
            .collect();

        for (name, _, _) in &valid_rows {
            let is_existing = existing_map.contains_key(name);
            let is_upserted = upserted_normalized.contains(name.as_str());

            let operation = if is_upserted {
                if is_existing { "updated" } else { "created" }
            } else {
                "unchanged"
            };

            results.push(LaborProcessImportRowResult {
                row_number: *seen_names.get(name).unwrap_or(&0),
                process_name: name.clone(),
                operation: operation.to_string(),
                error_message: String::new(),
            });
        }

        Ok(LaborProcessImportResult {
            success_count: to_upsert.len() as i32,
            failure_count: 0,
            skip_count,
            results,
            affected_bom_count,
        })
    }

    async fn export_processes_to_bytes(&self) -> Result<Vec<u8>> {
        let processes = LaborProcessRepo::list_all(&self.pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        for (col, header) in LABOR_PROCESS_EXCEL_COLUMNS.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, p) in processes.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &p.name)?;
            worksheet.write_number(row_num, 1, p.unit_price.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_string(row_num, 2, p.remark.as_deref().unwrap_or(""))?;
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }
}

fn to_member_tuples(members: &[LaborProcessGroupMemberInput]) -> Vec<(i64, i32)> {
    members.iter().map(|m| (m.process_id, m.sort_order)).collect()
}

fn row_error(row_number: i32, process_name: String, msg: impl Into<String>) -> LaborProcessImportRowResult {
    LaborProcessImportRowResult {
        row_number,
        process_name,
        operation: "error".to_string(),
        error_message: msg.into(),
    }
}

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "工序名称")]
    name: String,
    #[serde(rename = "单价", deserialize_with = "deserialize_price")]
    unit_price: Option<Decimal>,
    #[serde(rename = "备注")]
    remark: Option<String>,
}

fn deserialize_price<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
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

    #[test]
    fn test_deserialize_price_valid() {
        #[derive(Deserialize)]
        struct Row {
            #[serde(deserialize_with = "deserialize_price")]
            price: Option<Decimal>,
        }
        let row: Row = serde_json::from_str("{\"price\": \"15.50\"}").unwrap();
        assert_eq!(row.price, Some(Decimal::from_str_exact("15.50").unwrap()));
    }

    #[test]
    fn test_deserialize_price_empty() {
        #[derive(Deserialize)]
        struct Row {
            #[serde(deserialize_with = "deserialize_price")]
            price: Option<Decimal>,
        }
        let row: Row = serde_json::from_str("{\"price\": \"\"}").unwrap();
        assert_eq!(row.price, None);
    }

    #[test]
    fn test_deserialize_price_null() {
        #[derive(Deserialize)]
        struct Row {
            #[serde(deserialize_with = "deserialize_price")]
            price: Option<Decimal>,
        }
        let row: Row = serde_json::from_str("{\"price\": null}").unwrap();
        assert_eq!(row.price, None);
    }

    #[test]
    fn test_deserialize_price_invalid() {
        #[derive(Deserialize)]
        struct Row {
            #[serde(deserialize_with = "deserialize_price")]
            price: Option<Decimal>,
        }
        let result = serde_json::from_str::<Row>("{\"price\": \"abc\"}");
        assert!(result.is_err());
    }
}