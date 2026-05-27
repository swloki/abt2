use axum::body::Body;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::auth::session::CURRENT_USER_KEY;
use crate::state::AppState;

const LOGIN_PATH: &str = "/login";

pub async fn auth_middleware(
    State(_state): State<AppState>,
    session: Session,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Skip auth for public static assets
    let path = request.uri().path();
    if path.starts_with("/static") || path.starts_with("/favicon") {
        return next.run(request).await;
    }

    // Try to get claims from session
    match session.get::<abt_core::shared::identity::model::Claims>(CURRENT_USER_KEY).await {
        Ok(Some(_claims)) => next.run(request).await,
        Ok(None) => Redirect::to(LOGIN_PATH).into_response(),
        Err(e) => {
            tracing::error!("Session read error: {e}");
            Redirect::to(LOGIN_PATH).into_response()
        }
    }
}
