use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_approval_rules;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules")]
pub struct ApprovalRulesPath;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            ApprovalRulesPath::PATH,
            get(purchase_approval_rules::get_approval_rules)
                .post(purchase_approval_rules::create_rule),
        )
        .route(
            "/admin/purchase/approval-rules/{id}/delete",
            post(purchase_approval_rules::delete_rule),
        )
}
