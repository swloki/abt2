use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_approval_rules;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules")]
pub struct ApprovalRulesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules/create")]
pub struct RuleCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules/{id}")]
pub struct RuleEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules/{id}/delete")]
pub struct RuleDeletePath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            ApprovalRulesPath::PATH,
            get(purchase_approval_rules::get_list).post(purchase_approval_rules::create_rule),
        )
        .route(
            RuleCreatePath::PATH,
            get(purchase_approval_rules::get_create_modal),
        )
        .route(
            RuleEditPath::PATH,
            get(purchase_approval_rules::get_edit_modal)
                .post(purchase_approval_rules::update_rule),
        )
        .route(
            RuleDeletePath::PATH,
            post(purchase_approval_rules::delete_rule),
        )
}
