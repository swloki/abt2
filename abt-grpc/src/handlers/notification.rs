//! Notification gRPC Handler — 委托给 abt-core NotificationService

use abt_core::shared::notification::model::{NotificationQuery, NotificationType};
use abt_core::shared::notification::NotificationService;
use abt_core::shared::types::ServiceContext;
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_notification_service_server::AbtNotificationService as GrpcNotificationService,
    *,
};
use crate::handlers::GrpcResult;
use crate::handlers::domain_to_status;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

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

fn parse_notification_type(s: &str) -> Option<NotificationType> {
    match s {
        "system" | "System" | "1" => Some(NotificationType::System),
        "business" | "Business" | "2" => Some(NotificationType::Business),
        "alert" | "Alert" | "3" => Some(NotificationType::Alert),
        _ => None,
    }
}

fn notification_type_to_string(t: NotificationType) -> String {
    match t {
        NotificationType::System => "system".to_string(),
        NotificationType::Business => "business".to_string(),
        NotificationType::Alert => "alert".to_string(),
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

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let query = NotificationQuery {
            notification_type: req.r#type.as_deref().and_then(parse_notification_type),
            is_read: req.is_read,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(20),
        };

        let result = srv
            .list_notifications(ctx, query)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(ListNotificationsResponse {
            items: result
                .items
                .into_iter()
                .map(notification_to_proto)
                .collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
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

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        srv.mark_read(ctx, req.notification_id)
            .await
            .map(|_| true)
            .or_else(|e| match e {
                abt_core::shared::types::DomainError::NotFound(_) => Ok(false),
                other => Err(other),
            })
            .map_err(domain_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn mark_all_as_read(
        &self,
        request: Request<MarkAllAsReadRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.notification_service();

        let notification_type = req
            .r#type
            .as_deref()
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(parse_notification_type);

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let count = srv
            .mark_all_read(ctx, notification_type)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(BoolResponse { value: count > 0 }))
    }

    async fn get_unread_count(
        &self,
        request: Request<GetUnreadCountRequest>,
    ) -> GrpcResult<GetUnreadCountResponse> {
        let auth = extract_auth(&request)?;
        let state = AppState::get().await;
        let srv = state.notification_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let total = srv.get_unread_count(ctx).await.map_err(domain_to_status)?;

        // abt-core returns only total count (no by-type breakdown yet)
        Ok(Response::new(GetUnreadCountResponse {
            total,
            by_type: std::collections::HashMap::new(),
        }))
    }
}

fn notification_to_proto(n: abt_core::shared::notification::model::Notification) -> NotificationResponse {
    NotificationResponse {
        notification_id: n.notification_id,
        r#type: notification_type_to_string(n.notification_type),
        title: n.title,
        content: n.content,
        related_type: n.related_type,
        related_id: n.related_id,
        is_read: n.is_read,
        read_at: n.read_at.map(|d| d.to_rfc3339()),
        created_at: n.created_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
        metadata: None,
    }
}
