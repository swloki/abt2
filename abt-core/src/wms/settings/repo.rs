use sqlx::FromRow;

use super::model::{UpdateWmsSettingsReq, WmsSettings};
use crate::shared::types::Result;

pub struct WmsSettingsRepo;

impl WmsSettingsRepo {
    /// 读取单行设置（id=1）。表迁移时已保证该行存在。
    pub async fn get(executor: &mut sqlx::postgres::PgConnection) -> Result<WmsSettings> {
        let row = sqlx::query(
            r#"
            SELECT id, cycle_count_variance_threshold, created_at, updated_at
            FROM wms_settings
            WHERE id = 1
            "#,
        )
        .fetch_one(&mut *executor)
        .await?;

        Ok(WmsSettings::from_row(&row)?)
    }

    pub async fn update(
        executor: &mut sqlx::postgres::PgConnection,
        req: &UpdateWmsSettingsReq,
    ) -> Result<WmsSettings> {
        let row = sqlx::query(
            r#"
            UPDATE wms_settings
            SET cycle_count_variance_threshold = $1, updated_at = NOW()
            WHERE id = 1
            RETURNING id, cycle_count_variance_threshold, created_at, updated_at
            "#,
        )
        .bind(req.cycle_count_variance_threshold)
        .fetch_one(&mut *executor)
        .await?;

        Ok(WmsSettings::from_row(&row)?)
    }
}
