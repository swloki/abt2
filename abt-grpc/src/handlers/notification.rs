//! Notification gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_notification_service_server::AbtNotificationService as GrpcNotificationService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

use abt::NotificationService;

pub struct NotificationHandler;

impl NotificationHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NotificationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcNotificationService for NotificationHandler {
    async fn list_notifications(
        &self,
        request: Request<ListNotificationsRequest>,
    ) -> GrpcResult<ListNotificationsResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.notification_service();

        let query = abt::NotificationQuery {
            notification_type: req.r#type.filter(|s| !s.is_empty()),
            is_read: req.is_read,
            start_time: req.start_time.filter(|s| !s.is_empty()),
            end_time: req.end_time.filter(|s| !s.is_empty()),
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(20),
        };

        let (items, total) = srv
            .list_notifications(auth.user_id, &query)
            .await
            .map_err(error::err_to_status)?;

        let page = query.page;
        let page_size = query.page_size;

        Ok(Response::new(ListNotificationsResponse {
            items: items.into_iter().map(notification_to_proto).collect(),
            total: total as u64,
            page,
            page_size,
        }))
    }

    async fn mark_as_read(
        &self,
        request: Request<MarkAsReadRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.notification_service();

        let found = srv
            .mark_as_read(req.notification_id, auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: found }))
    }

    async fn mark_all_as_read(
        &self,
        request: Request<MarkAllAsReadRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.notification_service();

        let notification_type = req.r#type.as_deref().and_then(|s| if s.is_empty() { None } else { Some(s) });

        let count = srv
            .mark_all_as_read(auth.user_id, notification_type)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: count > 0 }))
    }

    async fn get_unread_count(
        &self,
        request: Request<GetUnreadCountRequest>,
    ) -> GrpcResult<GetUnreadCountResponse> {
        let auth = extract_auth(&request)?;
        let state = AppState::get().await;
        let srv = state.notification_service();

        let (total, by_type) = srv
            .get_unread_count(auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        let by_type_map: std::collections::HashMap<String, i64> = by_type
            .into_iter()
            .map(|t| (t.notification_type, t.count))
            .collect();

        Ok(Response::new(GetUnreadCountResponse {
            total,
            by_type: by_type_map,
        }))
    }
}

fn notification_to_proto(n: abt::Notification) -> NotificationResponse {
    NotificationResponse {
        notification_id: n.notification_id,
        r#type: n.notification_type,
        title: n.title,
        content: n.content,
        related_type: n.related_type,
        related_id: n.related_id,
        is_read: n.is_read,
        read_at: n.read_at,
        created_at: n.created_at,
        metadata: n.metadata.map(|v| v.to_string()),
    }
}
