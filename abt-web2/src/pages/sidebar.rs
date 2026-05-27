use tower_sessions::Session;

use crate::auth::session::CURRENT_USER_KEY;
use crate::layout::sidebar::sidebar_body_fragment;
use crate::routes::sidebar::SidebarBodyPath;

// ── Handler ──

pub async fn get_sidebar_body(
    path: SidebarBodyPath,
    session: Session,
) -> axum::response::Html<String> {
    let claims = session
        .get::<abt_core::shared::identity::model::Claims>(CURRENT_USER_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| abt_core::shared::identity::model::Claims {
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
        });

    let fragment = sidebar_body_fragment(&claims, &path.module);
    axum::response::Html(fragment.into_string())
}
