//! BOM 人工工序服务实现

use anyhow::Result;
use async_trait::async_trait;
use calamine::{Data, Reader, Xlsx, open_workbook};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::LaborProcessService;
use crate::models::{
    BomLaborProcess, CreateLaborProcessRequest, ImportResult, ListLaborProcessRequest,
    UpdateLaborProcessRequest,
};
use crate::repositories::{Executor, LaborProcessRepo};

pub struct LaborProcessServiceImpl {
    pool: PgPool,
}

impl LaborProcessServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 从 Excel Cell 获取字符串值
fn get_cell_string(cell: Option<&Data>) -> String {
    match cell {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Int(i)) => i.to_string(),
        Some(Data::Float(f)) => f.to_string(),
        Some(Data::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// 从 Excel Cell 获取浮点数值
fn get_cell_float(cell: Option<&Data>) -> f64 {
    match cell {
        Some(Data::Float(f)) => *f,
        Some(Data::Int(i)) => *i as f64,
        _ => 0.0,
    }
}

#[async_trait]
impl LaborProcessService for LaborProcessServiceImpl {
    async fn create(&self, req: CreateLaborProcessRequest, executor: Executor<'_>) -> Result<i64> {
        LaborProcessRepo::insert(executor, &req).await
    }

    async fn update(&self, req: UpdateLaborProcessRequest, executor: Executor<'_>) -> Result<()> {
        LaborProcessRepo::update(executor, &req).await
    }

    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64> {
        LaborProcessRepo::delete(executor, id, product_code).await
    }

    async fn list(&self, req: ListLaborProcessRequest) -> Result<(Vec<BomLaborProcess>, i64)> {
        let page = req.page.unwrap_or(1).max(1);
        let page_size = req.page_size.unwrap_or(50).clamp(1, 100);

        let items =
            LaborProcessRepo::find_by_product_code(&self.pool, &req.product_code, page, page_size)
                .await?;
        let total = LaborProcessRepo::count_by_product_code(&self.pool, &req.product_code).await?;

        Ok((items, total))
    }

    async fn import(&self, file_path: &str, executor: Executor<'_>) -> Result<ImportResult> {
        // 尝试直接打开文件，让 open_workbook 处理不存在的错误
        let mut workbook: Xlsx<_> =
            open_workbook(file_path).map_err(|e| anyhow::anyhow!("Failed to open Excel: {}", e))?;

        let sheet_names = workbook.sheet_names().to_vec();
        if sheet_names.is_empty() {
            return Ok(ImportResult {
                success_count: 0,
                fail_count: 1,
                errors: vec!["Excel has no sheets".to_string()],
            });
        }

        let range = workbook
            .worksheet_range(&sheet_names[0])
            .map_err(|e| anyhow::anyhow!("Failed to read sheet: {}", e))?;

        // 按产品编码分组：(product_code) -> Vec<(name, unit_price, quantity, sort_order, remark)>
        let mut process_map: std::collections::HashMap<
            String,
            Vec<(String, Decimal, Decimal, i32, Option<String>)>,
        > = std::collections::HashMap::new();
        let mut row_errors: Vec<String> = Vec::new();

        // 跳过第一行（表头），从第二行开始
        for (idx, row) in range.rows().enumerate().skip(1) {
            let row_num = idx + 1;

            // 解析各列
            let product_code = get_cell_string(row.first());
            let name = get_cell_string(row.get(1));
            let unit_price_str = get_cell_string(row.get(2));
            let quantity_str = get_cell_string(row.get(3));
            let sort_order = get_cell_float(row.get(4)) as i32;
            let remark = if let Some(cell) = row.get(5) {
                let s = get_cell_string(Some(cell));
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            };

            // 校验必填字段
            if product_code.is_empty() {
                row_errors.push(format!("Row {}: product_code is required", row_num));
                continue;
            }
            if name.is_empty() {
                row_errors.push(format!("Row {}: name is required", row_num));
                continue;
            }
            let Ok(unit_price) = unit_price_str.parse::<Decimal>() else {
                row_errors.push(format!("Row {}: invalid unit_price", row_num));
                continue;
            };
            let Ok(quantity) = quantity_str.parse::<Decimal>() else {
                row_errors.push(format!("Row {}: invalid quantity", row_num));
                continue;
            };

            process_map
                .entry(product_code)
                .or_default()
                .push((name, unit_price, quantity, sort_order, remark));
        }

        // 如果有行解析错误，返回但不继续（数据可能不完整）
        if !row_errors.is_empty() {
            return Ok(ImportResult {
                success_count: 0,
                fail_count: row_errors.len() as u64,
                errors: row_errors,
            });
        }

        // 执行导入（按产品编码分组导入）
        // 注意：如果任何产品编码的删除或插入失败，整个事务将回滚
        let mut total_success = 0u64;

        for (product_code, items) in process_map {
            if items.is_empty() {
                continue;
            }

            // 先删除该产品编码的所有现有工序
            LaborProcessRepo::delete_by_product_code(executor, &product_code).await?;

            // 批量插入新工序
            LaborProcessRepo::batch_insert(executor, &product_code, &items).await?;

            total_success += items.len() as u64;
        }

        Ok(ImportResult {
            success_count: total_success,
            fail_count: 0,
            errors: vec![],
        })
    }
}
