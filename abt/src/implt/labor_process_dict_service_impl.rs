//! 工序字典服务实现

use anyhow::Result;
use async_trait::async_trait;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, LaborProcessDictRepo};
use crate::service::LaborProcessDictService;

/// 工序字典 Excel 列头
const DICT_EXCEL_COLUMNS: &[&str] = &["工序编码", "工序名称", "说明", "排序"];

pub struct LaborProcessDictServiceImpl {
    pool: PgPool,
}

impl LaborProcessDictServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LaborProcessDictService for LaborProcessDictServiceImpl {
    // ========================================================================
    // 查询
    // ========================================================================

    async fn list(&self, query: ListLaborProcessDictQuery) -> Result<(Vec<LaborProcessDict>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessDictRepo::find_all(&self.pool, kw, page, page_size).await?;
        let total = LaborProcessDictRepo::count_all(&self.pool, kw).await?;
        Ok((items, total))
    }

    // ========================================================================
    // 写入
    // ========================================================================

    async fn create(&self, req: CreateLaborProcessDictReq, executor: Executor<'_>) -> Result<i64> {
        // 通过 SEQUENCE 生成编码，天然并发安全
        let next_val: i64 = sqlx::query_scalar(
            "SELECT nextval('labor_process_dict_code_seq')"
        )
        .fetch_one(&self.pool)
        .await?;
        let code = format!("{:05}", next_val);

        LaborProcessDictRepo::insert(
            executor,
            &code,
            &req.name,
            req.description.as_deref(),
            req.sort_order,
        )
        .await
    }

    async fn update(&self, req: UpdateLaborProcessDictReq, executor: Executor<'_>) -> Result<()> {
        // 检查记录是否存在
        LaborProcessDictRepo::find_by_id(&self.pool, req.id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工序字典".to_string(),
                id: req.id.to_string(),
            })?;

        LaborProcessDictRepo::update(
            executor,
            req.id,
            &req.name,
            req.description.as_deref(),
            req.sort_order,
        )
        .await
    }

    async fn delete(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        // 检查记录是否存在并获取编码
        let existing = LaborProcessDictRepo::find_by_id(&self.pool, id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工序字典".to_string(),
                id: id.to_string(),
            })?;

        // 检查是否被 routing_step 引用
        if LaborProcessDictRepo::exists_by_process_code(&self.pool, &existing.code).await? {
            return Err(common::error::ServiceError::BusinessValidation {
                message: format!("工序编码 '{}' 已被工艺路线引用，无法删除", existing.code),
            }
            .into());
        }

        LaborProcessDictRepo::delete(executor, id).await
    }

    async fn export_to_bytes(&self) -> Result<Vec<u8>> {
        let items = LaborProcessDictRepo::list_all(&self.pool).await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        for (col, header) in DICT_EXCEL_COLUMNS.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header)?;
        }

        for (row_idx, d) in items.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &d.code)?;
            worksheet.write_string(row_num, 1, &d.name)?;
            worksheet.write_string(row_num, 2, d.description.as_deref().unwrap_or(""))?;
            worksheet.write_number(row_num, 3, d.sort_order as f64)?;
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }
}
