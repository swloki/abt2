use sqlx::Row;

use crate::shared::types::Result;

use super::model::PurchaseSettings;

pub struct PurchaseSettingsRepo;

impl PurchaseSettingsRepo {
    /// 获取单行配置（id=1）
    pub async fn get(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<PurchaseSettings> {
        let row = sqlx::query(
            r#"
            SELECT id, over_delivery_allowance_pct, over_shortage_allowance_pct,
                   maintain_same_rate, po_required_for_receipt, receipt_required_for_invoice,
                   default_currency_code, default_tax_rate_id, created_at, updated_at
            FROM purchase_settings
            WHERE id = 1
            "#,
        )
        .fetch_one(&mut *executor)
        .await?;

        Ok(PurchaseSettings {
            id: row.try_get("id")?,
            over_delivery_allowance_pct: row.try_get("over_delivery_allowance_pct")?,
            over_shortage_allowance_pct: row.try_get("over_shortage_allowance_pct")?,
            maintain_same_rate: row.try_get("maintain_same_rate")?,
            po_required_for_receipt: row.try_get("po_required_for_receipt")?,
            receipt_required_for_invoice: row.try_get("receipt_required_for_invoice")?,
            default_currency_code: row.try_get("default_currency_code")?,
            default_tax_rate_id: row.try_get("default_tax_rate_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    /// 更新配置（动态字段）
    pub async fn update(
        executor: &mut sqlx::postgres::PgConnection,
        req: &super::model::UpdatePurchaseSettingsRequest,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_settings SET
                over_delivery_allowance_pct = COALESCE($2, over_delivery_allowance_pct),
                over_shortage_allowance_pct = COALESCE($3, over_shortage_allowance_pct),
                maintain_same_rate = COALESCE($4, maintain_same_rate),
                po_required_for_receipt = COALESCE($5, po_required_for_receipt),
                receipt_required_for_invoice = COALESCE($6, receipt_required_for_invoice),
                default_currency_code = COALESCE($7, default_currency_code),
                default_tax_rate_id = COALESCE($8, default_tax_rate_id),
                updated_at = NOW()
            WHERE id = 1
            "#,
        )
        .bind(req.over_delivery_allowance_pct)
        .bind(req.over_shortage_allowance_pct)
        .bind(req.maintain_same_rate)
        .bind(req.po_required_for_receipt)
        .bind(req.receipt_required_for_invoice)
        .bind(&req.default_currency_code)
        .bind(req.default_tax_rate_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
