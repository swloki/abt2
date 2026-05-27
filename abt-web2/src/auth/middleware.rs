use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::COOKIE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::auth::session::Session;
use crate::state::AppState;

/// Paths that don't require authentication.
const PUBLIC_PATHS: &[&str] = &["/login", "/static", "/favicon"];

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Skip auth for public paths
    if PUBLIC_PATHS.iter().any(|p| path.starts_with(p)) || path == "/logout" {
        return next.run(req).await;
    }

    // Extract token from cookie
    let token = req
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|c| c.trim().strip_prefix("token="))
        });

    let token = match token {
        Some(t) => t,
        None => return Redirect::to("/login").into_response(),
    };

    // Validate JWT
    match decode::<abt_core::shared::identity::model::Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => {
            let session = Session {
                claims: data.claims,
            };
            req.extensions_mut().insert(session);
            next.run(req).await
        }
        Err(_) => Redirect::to("/login").into_response(),
    }
}
