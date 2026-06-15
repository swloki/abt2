//! BOM 列表全量导出实现

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use super::helpers::write_headers;
use crate::master_data::bom::model::BomStatus;

/// BOM 列表导出列定义
const BOMS_LIST_HEADERS: [&str; 7] = [
    "BOM ID", "BOM 名称", "产品编号", "BOM 分类", "版本", "状态", "创建时间",
];

#[derive(sqlx::FromRow)]
struct BomListRow {
    bom_id: i64,
    bom_name: String,
    product_code: Option<String>,
    bom_category_name: Option<String>,
    version: i32,
    status: BomStatus,
    create_at: DateTime<Utc>,
}

/// BOM 列表 Excel 导出器
pub struct BomsListExporter {
    pool: PgPool,
}

impl BomsListExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出全部 BOM 列表到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query_as::<sqlx::Postgres, BomListRow>(
            r#"
            SELECT b.bom_id, b.bom_name,
                (SELECT COALESCE(bn.product_code, p.product_code)
                 FROM bom_nodes bn
                 LEFT JOIN products p ON p.product_id = bn.product_id
                 WHERE bn.bom_id = b.bom_id AND bn.parent_id = 0
                 LIMIT 1) AS product_code,
                bc.bom_category_name,
                b.version, b.status, b.create_at
            FROM boms b
            LEFT JOIN bom_categories bc ON bc.bom_category_id = b.bom_category_id
            WHERE b.deleted_at IS NULL
            ORDER BY b.bom_id DESC
            "#,
        )
        .fetch_all(&mut *conn)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &BOMS_LIST_HEADERS)?;

        for (row_idx, r) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_number(row_num, 0, r.bom_id as f64)?;
            worksheet.write_string(row_num, 1, &r.bom_name)?;
            worksheet.write_string(row_num, 2, r.product_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 3, r.bom_category_name.as_deref().unwrap_or(""))?;
            worksheet.write_number(row_num, 4, r.version as f64)?;
            worksheet.write_string(
                row_num,
                5,
                if r.status == BomStatus::Published { "已发布" } else { "草稿" },
            )?;
            worksheet.write_string(row_num, 6, r.create_at.format("%Y-%m-%d %H:%M").to_string())?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
