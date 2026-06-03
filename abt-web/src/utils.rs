use std::collections::HashMap;

use abt_core::master_data::customer::CustomerService;
use abt_core::shared::identity::{model::Claims, PermissionService};
use abt_core::shared::types::{PgExecutor, PgPoolConn, ServiceContext};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum::extract::FromRequestParts;
use serde::{Deserialize, de};
use tower_sessions::Session;

use crate::auth::session::CURRENT_USER_KEY;
use crate::errors::WebError;
use crate::state::AppState;

pub fn empty_as_none<'de, D, T>(de: D) -> std::result::Result<Option<T>, D::Error>
where
    D: de::Deserializer<'de>,
    T: std::str::FromStr,
{
    let s: Option<String> = Option::deserialize(de)?;
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(v) => v.parse::<T>().map(Some).map_err(|_| {
            de::Error::custom(format!("cannot parse '{v}'"))
        }),
    }
}

pub async fn resolve_customer_names<S: CustomerService>(
    svc: &S,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    ids: impl IntoIterator<Item = i64>,
) -> HashMap<i64, String> {
    let unique: Vec<i64> = ids.into_iter().collect();
    if unique.is_empty() {
        return HashMap::new();
    }
    match svc.get_by_ids(ctx, db, &unique).await {
        Ok(customers) => customers.into_iter()
            .map(|c| (c.id, c.name))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

fn guest_claims() -> Claims {
    Claims {
        sub: 0,
        username: "未知用户".into(),
        display_name: "未知用户".into(),
        system_role: "user".into(),
        role_ids: vec![],
        role_codes: vec![],
        department_ids: vec![],
        iss: String::new(),
        exp: 0,
        iat: 0,
    }
}

pub struct RequestContext {
    pub claims: Claims,
    pub conn: PgPoolConn,
    pub state: AppState,
    pub service_ctx: ServiceContext,
    pub headers: HeaderMap,
}

impl FromRequestParts<AppState> for RequestContext {
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let headers = std::mem::take(&mut parts.headers);

        let session = parts.extensions.remove::<Session>()
            .expect("Session not found. Is SessionManagerLayer installed?");

        let claims = session.get::<Claims>(CURRENT_USER_KEY).await
            .ok()
            .flatten()
            .unwrap_or_else(guest_claims);

        let conn = state.pool.acquire().await
            .map_err(abt_core::shared::types::DomainError::from)?;

        let service_ctx = ServiceContext::new(claims.sub);

        Ok(RequestContext {
            claims,
            conn,
            state: state.clone(),
            service_ctx,
            headers,
        })
    }
}

impl RequestContext {
    pub async fn has_permission(&self, resource: &str, action: &str) -> bool {
        self.state.permission_service()
            .check_permission(self.claims.is_super_admin(), &self.claims.role_ids, resource, action)
            .await
            .unwrap_or(false)
    }
    pub fn is_htmx(&self) -> bool {
        self.headers.get("HX-Request").is_some()
    }
}
