//! 采购需求池 — DemandCreated 事件处理器

use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::warn;

use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::notification::{new_notification_service, service::NotificationService};
use crate::shared::notification::model::{BatchNotificationReq, NotificationType};
use crate::shared::types::{Result, ServiceContext};

use super::repo::PurchaseDemandRepo;

// TODO: 从系统角色配置中获取实际值
const PURCHASE_ROLE_ID: i64 = 3;

/// 采购需求创建 Handler — 监听 DemandCreated 事件，发送通知给采购角色
pub struct PurchaseDemandCreatedHandler {
    pool: PgPool,
}

impl PurchaseDemandCreatedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for PurchaseDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let acquire_channel = event.payload["acquire_channel"].as_i64();

        // 只处理外购需求（acquire_channel = 2）
        if acquire_channel != Some(2) {
            return Ok(());
        }

        let demand_id = event.aggregate_id;

        // 回查视图获取需求数据（包含 product_name, order_no）
        let mut conn = self.pool.acquire().await
            .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;

        let detail = match PurchaseDemandRepo::find_detail_by_id(&mut conn, demand_id).await? {
            Some(d) => d,
            None => {
                // 需求不存在或不在视图中 — 记录 Warning
                warn!(demand_id, "Demand not found in v_purchase_demands, skipping notification");
                return Ok(());
            }
        };

        // 防御事件乱序：status 不是 Pending 则跳过
        if detail.demand_status != 1 {
            return Ok(());
        }

        // 发送通知给采购角色
        let ctx = ServiceContext::system();
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(
            &ctx,
            &mut conn,
            PURCHASE_ROLE_ID,
            BatchNotificationReq {
                notification_type: NotificationType::Business,
                title: "新的外购需求待处理".into(),
                content: Some(format!(
                    "产品: {} ({}) × {}, 来源订单: {}",
                    detail.product_name, detail.product_code, detail.quantity, detail.order_no.as_deref().unwrap_or("—")
                )),
                related_type: Some("demand".into()),
                related_id: Some(demand_id),
            },
        ).await?;

        Ok(())
    }

    fn name(&self) -> &str {
        "purchase_demand_created"
    }
}
