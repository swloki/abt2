//! 应收应付台账明细 Excel 导出实现
//!
//! 把台账展开到产品行项目级（每产品一行，含数量/单价/行金额），
//! 供「应付/应收台账明细表」导出。数据来自 `ArApLedgerRepo::query_details`。

use anyhow::{Context, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::fms::ar_ap::model::ArApLedgerFilter;
use crate::fms::ar_ap::repo::ArApLedgerRepo;
use crate::fms::enums::CounterpartyType;

use super::helpers::write_headers;

/// 应收应付台账明细 Excel 导出器
pub struct LedgerDetailExporter {
    pool: PgPool,
    party_type: CounterpartyType,
    filter: ArApLedgerFilter,
}

impl LedgerDetailExporter {
    pub fn new(pool: PgPool, party_type: CounterpartyType, filter: ArApLedgerFilter) -> Self {
        Self {
            pool,
            party_type,
            filter,
        }
    }

    /// 导出台账明细到 Excel 字节数据
    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let rows = ArApLedgerRepo::query_details(&mut conn, &self.filter).await?;

        // 「上游单号」列名按方向动态：应付=采购单号，应收=销售单号
        let upstream_header: &'static str = match self.party_type {
            CounterpartyType::Supplier => "采购单号",
            CounterpartyType::Customer => "销售单号",
            _ => "上游单号",
        };

        let headers = [
            "往来方",
            "发生单号",
            upstream_header,
            "产品编码",
            "产品名称",
            "数量",
            "单价",
            "行金额",
            "发生日期",
        ];

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        write_headers(worksheet, &headers)?;

        for (row_idx, r) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            worksheet.write_string(row_num, 0, &r.party_name)?;
            worksheet.write_string(row_num, 1, &r.source_doc_no)?;
            worksheet.write_string(row_num, 2, r.upstream_doc_no.as_deref().unwrap_or(""))?;
            worksheet.write_string(row_num, 3, &r.product_code)?;
            worksheet.write_string(row_num, 4, &r.product_name)?;
            worksheet.write_number(
                row_num,
                5,
                r.quantity.to_f64().context("Decimal 转 f64 失败")?,
            )?;
            worksheet.write_number(
                row_num,
                6,
                r.unit_price.to_f64().context("Decimal 转 f64 失败")?,
            )?;
            worksheet.write_number(
                row_num,
                7,
                r.line_amount.to_f64().context("Decimal 转 f64 失败")?,
            )?;
            worksheet.write_string(row_num, 8, &r.transaction_date.format("%Y-%m-%d").to_string())?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}
