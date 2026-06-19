//! Test harness for abt-web handler integration tests.
//!
//! Builds a full Router with session + auth middleware, using a MemoryStore.
//! Pre-injects super-admin Claims into every session so auth middleware passes.

use std::sync::{Arc, Once};

use abt_core::shared::identity::model::Claims;
use abt_web::state::AppState;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use tower_sessions::{session_store::SessionStore, Session, SessionManagerLayer};

static TRACING_INIT: Once = Once::new();
/// Test fixture wrapping an AppState + MemoryStore session layer.
pub struct TestApp {
    pub state: AppState,
    session_store: tower_sessions::MemoryStore,
}

impl TestApp {
    /// Connect to the real DB (from DATABASE_URL env), build AppState with
    /// super-admin permission cache.
    pub async fn new() -> Self {
        // Initialize tracing subscriber once for all tests
        TRACING_INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,abt_core=debug,abt_web=debug")),
                )
                .try_init();
        });

        dotenvy::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = abt_core::shared::types::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("failed to connect to DB");

        let permission_cache =
            Arc::new(abt_core::shared::identity::RolePermissionCache::new(pool.clone()));
        permission_cache
            .load(&pool)
            .await
            .expect("failed to load permission cache");

        let state = AppState::from_pool(pool, permission_cache);
        let session_store = tower_sessions::MemoryStore::default();

        Self { state, session_store }
    }

    /// Build a test Router: full routes + session layer + claims injection.
    ///
    /// Request flow: session_layer (creates Session) → inject_claims (populates
    /// Claims into Session) → auth_middleware (reads Claims, passes) → handler.
    pub fn router(&self) -> axum::Router {
        let claims = test_claims();
        let session_layer = SessionManagerLayer::new(self.session_store.clone()).with_secure(false);

        abt_web::routes::router(self.state.clone())
            .layer(axum::middleware::from_fn(move |mut req: axum::extract::Request<Body>, next: axum::middleware::Next| {
                let claims = claims.clone();
                async move {
                    if let Some(session) = req.extensions_mut().remove::<Session>() {
                        let _ = session
                            .insert(abt_web::auth::session::CURRENT_USER_KEY, &claims)
                            .await;
                        req.extensions_mut().insert(session);
                    }
                    next.run(req).await
                }
            }))
            .layer(session_layer)
    }

    /// GET request as super-admin.
    pub async fn get(&self, uri: &str) -> TestResponse {
        self.send(Method::GET, uri, None, false).await
    }

    /// GET request with HTMX header.
    pub async fn get_htmx(&self, uri: &str) -> TestResponse {
        self.send(Method::GET, uri, None, true).await
    }

    /// POST request with form body.
    pub async fn post(&self, uri: &str, body: &str) -> TestResponse {
        self.send(Method::POST, uri, Some(body), false).await
    }

    /// POST request with HTMX header.
    pub async fn post_htmx(&self, uri: &str, body: &str) -> TestResponse {
        self.send(Method::POST, uri, Some(body), true).await
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
        is_htmx: bool,
    ) -> TestResponse {
        let router = self.router();
        let mut builder = Request::builder().method(method).uri(uri);

        if is_htmx {
            builder = builder.header("HX-Request", "true");
        }

        let req = if let Some(body_str) = body {
            builder =
                builder.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
            builder.body(Body::from(body_str.to_string())).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };

        let response = router.oneshot(req).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();

        TestResponse {
            status,
            headers,
            body: body_str,
        }
    }
}

/// Response wrapper for assertions.
pub struct TestResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: String,
}

impl TestResponse {
    pub fn is_ok(&self) -> bool {
        self.status == StatusCode::OK
    }

    pub fn is_redirect(&self) -> bool {
        self.status.is_redirection()
    }

    pub fn hx_redirect(&self) -> Option<&str> {
        self.headers.get("HX-Redirect").and_then(|v| v.to_str().ok())
    }

    pub fn body_contains(&self, needle: &str) -> bool {
        self.body.contains(needle)
    }
}

/// Super-admin Claims for testing.
fn test_claims() -> Claims {
    Claims {
        sub: 1,
        username: "admin".into(),
        display_name: "Admin".into(),
        system_role: "super_admin".into(),
        role_ids: vec![1],
        role_codes: vec!["super_admin".into()],
        department_ids: vec![],
        iss: "test".into(),
        exp: u64::MAX,
        iat: 0,
    }
}

// Suppress unused-import warning for SessionStore trait (needed for MemoryStore: SessionStore bound).
#[allow(dead_code)]
fn _assert_session_store<T: SessionStore + ?Sized>(_t: &T) {}
