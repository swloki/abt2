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

/// Deserializer that accepts either a single string or a sequence of strings.
/// Used for checkbox groups where 0..1 checked items send a single value,
/// but 2+ send a sequence.
pub fn multi_string<'de, D>(de: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    use serde::de::Visitor;
    struct MultiStringVisitor;
    impl<'de> Visitor<'de> for MultiStringVisitor {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("string or sequence of strings") }
        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Vec<String>, E> {
            if v.is_empty() { Ok(vec![]) } else { Ok(vec![v.to_string()]) }
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Vec<String>, A::Error> {
            let mut v = Vec::new();
            while let Some(item) = seq.next_element::<String>()? { if !item.is_empty() { v.push(item); } }
            Ok(v)
        }
    }
    de.deserialize_any(MultiStringVisitor)
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
    pub session: Session,
}

impl FromRequestParts<AppState> for RequestContext {
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let headers = parts.headers.clone();

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
            session,
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
    pub async fn nav_filter(&self) -> crate::layout::sidebar::NavFilter {
        let perms = self.state.permission_cache
            .get_merged_permissions(&self.claims.role_ids)
            .await;
        crate::layout::sidebar::NavFilter::new(self.claims.is_super_admin(), perms)
    }
}

/// Format a Decimal value by trimming trailing zeros (100.000000 → 100, 1.50 → 1.5)
pub fn fmt_qty(v: impl Into<rust_decimal::Decimal>) -> String {
    let d = v.into();
    let s = d.to_string();
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Format a Decimal as currency with 2 decimal places and ¥ prefix (e.g. ¥ 128,500.00)
pub fn fmt_amount(v: impl Into<rust_decimal::Decimal>) -> String {
    let d = v.into();
    let abs = d.abs();
    let formatted = format!("{:.2}", abs);
    // Add thousands separator
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts.get(1).unwrap_or(&"00");
    let int_with_sep = int_part.as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(",");
    if d.is_sign_negative() {
        format!("-¥ {}.{}", int_with_sep, dec_part)
    } else {
        format!("¥ {}.{}", int_with_sep, dec_part)
    }
}
