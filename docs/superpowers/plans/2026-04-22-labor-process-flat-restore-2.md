# Labor Process Flat Model Restore — Implementation Plan (Part 2)

Continued from `docs/superpowers/plans/2026-04-22-labor-process-flat-restore.md` (Tasks 1-3)

---

## Task 4: Repository

**Files:**
- Rewrite: `abt/src/repositories/labor_process_repo.rs`

- [ ] **Step 1: Write the new repository file**

Replace the entire content of `abt/src/repositories/labor_process_repo.rs` with:

```rust
//! 劳务工序数据访问层
//!
//! 提供扁平 bom_labor_process 表的 CRUD 和批量操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序仓库
pub struct LaborProcessRepo;

impl LaborProcessRepo {
    // ========================================================================
    // 查询
    // ========================================================================

    /// 按产品查询工序（支持按名称模糊搜索）
    pub async fn find_by_product_code(
        pool: &PgPool,
        product_code: &str,
        keyword: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<BomLaborProcess>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<BomLaborProcess> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
                 FROM bom_labor_process \
                 WHERE product_code = $1 AND name ILIKE $2 \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $3 OFFSET $4"
            )
            .bind(product_code)
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
                 FROM bom_labor_process \
                 WHERE product_code = $1 \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $2 OFFSET $3"
            )
            .bind(product_code)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 按产品统计工序数量（支持按名称模糊搜索）
    pub async fn count_by_product_code(
        pool: &PgPool,
        product_code: &str,
        keyword: Option<&str>,
    ) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM bom_labor_process WHERE product_code = $1 AND name ILIKE $2"
            )
            .bind(product_code)
            .bind(&pattern)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM bom_labor_process WHERE product_code = $1"
            )
            .bind(product_code)
            .fetch_one(pool)
            .await?
        };
        Ok(count)
    }

    // ========================================================================
    // 写入
    // ========================================================================

    /// 创建工序
    pub async fn insert(
        executor: Executor<'_>,
        product_code: &str,
        name: &str,
        unit_price: Decimal,
        quantity: Decimal,
        sort_order: i32,
        remark: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO bom_labor_process (product_code, name, unit_price, quantity, sort_order, remark)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#
        )
        .bind(product_code)
        .bind(name)
        .bind(unit_price)
        .bind(quantity)
        .bind(sort_order)
        .bind(remark)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工序
    pub async fn update(
        executor: Executor<'_>,
        id: i64,
        product_code: &str,
        name: &str,
        unit_price: Decimal,
        quantity: Decimal,
        sort_order: i32,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE bom_labor_process
            SET product_code = $1, name = $2, unit_price = $3, quantity = $4, sort_order = $5, remark = $6, updated_at = NOW()
            WHERE id = $7
            "#
        )
        .bind(product_code)
        .bind(name)
        .bind(unit_price)
        .bind(quantity)
        .bind(sort_order)
        .bind(remark)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 删除工序（验证 product_code 匹配）
    pub async fn delete(executor: Executor<'_>, id: i64, product_code: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM bom_labor_process WHERE id = $1 AND product_code = $2"
        )
        .bind(id)
        .bind(product_code)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // Excel 批量操作
    // ========================================================================

    /// 删除产品的所有工序（用于导入前清除）
    pub async fn delete_by_product_code(
        executor: Executor<'_>,
        product_code: &str,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM bom_labor_process WHERE product_code = $1"
        )
        .bind(product_code)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 批量插入工序（用于导入）
    /// items: (name, unit_price, quantity, sort_order, remark)
    pub async fn batch_insert(
        executor: Executor<'_>,
        product_code: &str,
        items: &[(String, Decimal, Decimal, i32, Option<String>)],
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            "INSERT INTO bom_labor_process (product_code, name, unit_price, quantity, sort_order, remark) "
        );
        builder.push_values(items.iter(), |mut b, (name, unit_price, quantity, sort_order, remark)| {
            b.push_bind(product_code);
            b.push_bind(name);
            b.push_bind(*unit_price);
            b.push_bind(*quantity);
            b.push_bind(*sort_order);
            b.push_bind(remark);
        });
        builder.build().execute(executor).await?;
        Ok(())
    }

    /// 查询产品的所有工序（用于导出，按 sort_order 排序）
    pub async fn list_all_by_product_code(
        pool: &PgPool,
        product_code: &str,
    ) -> Result<Vec<BomLaborProcess>> {
        let items = sqlx::query_as(
            "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
             FROM bom_labor_process \
             WHERE product_code = $1 \
             ORDER BY sort_order ASC, id ASC"
        )
        .bind(product_code)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p abt`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add abt/src/repositories/labor_process_repo.rs
git commit -m "refactor: rewrite labor process repo for flat model"
```

---

## Task 5: Service Trait

**Files:**
- Rewrite: `abt/src/service/labor_process_service.rs`

- [ ] **Step 1: Write the new service trait**

Replace the entire content of `abt/src/service/labor_process_service.rs` with:

```rust
//! 劳务工序服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序服务接口
#[async_trait]
pub trait LaborProcessService: Send + Sync {
    /// 搜索工序（按产品）
    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)>;

    /// 创建工序
    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序
    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工序
    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64>;

    /// 从 Excel 导入工序（清除旧的，批量插入新的）
    async fn import_from_excel(&self, product_code: &str, file_path: &str) -> Result<LaborProcessImportResult>;

    /// 导出工序到 Excel 字节流
    async fn export_to_bytes(&self, product_code: &str) -> Result<Vec<u8>>;
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p abt`
Expected: Build succeeds (impl not yet matching, may error — that's expected until Task 6)

- [ ] **Step 3: Commit**

```bash
git add abt/src/service/labor_process_service.rs
git commit -m "refactor: rewrite labor process service trait for flat model"
```

---

## Task 6: Service Implementation

**Files:**
- Rewrite: `abt/src/implt/labor_process_service_impl.rs`

- [ ] **Step 1: Write the new service implementation**

Replace the entire content of `abt/src/implt/labor_process_service_impl.rs` with:

```rust
//! 劳务工序服务实现

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
        product_code: &str,
        file_path: &str,
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

        let mut valid_rows: Vec<(String, Decimal, Decimal, i32, Option<String>)> = Vec::new();
        let mut results: Vec<LaborProcessImportRowResult> = Vec::new();
        let mut seen_names: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
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

            let quantity = row.quantity.unwrap_or(Decimal::ONE);
            if quantity < Decimal::ZERO {
                failure_count += 1;
                results.push(row_error(row_number, name, "数量不能为负数"));
                continue;
            }

            let sort_order = row.sort_order.unwrap_or(row_number);

            if let Some(&first_row) = seen_names.get(&name) {
                failure_count += 1;
                results.push(row_error(row_number, name.clone(), format!("与第 {first_row} 行的工序名称重复")));
                continue;
            }
            seen_names.insert(name.clone(), row_number);

            valid_rows.push((name, unit_price, quantity, sort_order, row.remark));
        }

        if failure_count > 0 {
            return Ok(LaborProcessImportResult {
                success_count: 0,
                failure_count,
                results,
            });
        }

        // 事务：清除旧的 + 批量插入新的
        let mut tx = self.pool.begin().await?;
        LaborProcessRepo::delete_by_product_code(&mut tx, product_code).await?;
        LaborProcessRepo::batch_insert(&mut tx, product_code, &valid_rows).await?;
        tx.commit().await?;

        let success_count = valid_rows.len() as i32;
        for (name, _, _, _, _) in &valid_rows {
            results.push(LaborProcessImportRowResult {
                row_number: *seen_names.get(name).unwrap_or(&0),
                process_name: name.clone(),
                operation: "created".to_string(),
                error_message: String::new(),
            });
        }

        Ok(LaborProcessImportResult {
            success_count,
            failure_count: 0,
            results,
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
            worksheet.write_string(row_num, 0, &p.name)?;
            worksheet.write_number(row_num, 1, p.unit_price.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 2, p.quantity.to_f64().context("Decimal 转 f64 失败")?)?;
            worksheet.write_number(row_num, 3, p.sort_order as f64)?;
            worksheet.write_string(row_num, 4, p.remark.as_deref().unwrap_or(""))?;
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

#[derive(Debug, Deserialize)]
struct ExcelRow {
    #[serde(rename = "工序名称")]
    name: String,
    #[serde(rename = "单价", deserialize_with = "deserialize_decimal")]
    unit_price: Option<Decimal>,
    #[serde(rename = "数量", deserialize_with = "deserialize_decimal_opt")]
    quantity: Option<Decimal>,
    #[serde(rename = "排序", deserialize_with = "deserialize_int_opt")]
    sort_order: Option<i32>,
    #[serde(rename = "备注")]
    remark: Option<String>,
}

fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p abt`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add abt/src/implt/labor_process_service_impl.rs
git commit -m "refactor: rewrite labor process service impl for flat model"
```

---

Continued in `docs/superpowers/plans/2026-04-22-labor-process-flat-restore-3.md` (Task 7: Handler + Final Verification)
