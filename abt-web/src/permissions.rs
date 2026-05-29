use crate::errors::WebError;
use crate::utils::RequestContext;
use abt_core::shared::identity::PermissionService;
use abt_core::shared::types::DomainError;

/// Runtime permission check used by `#[require_permission]`.
pub async fn check_permission(
    ctx: &RequestContext,
    resource: &str,
    action: &str,
) -> Result<(), WebError> {
    let svc = ctx.state.permission_service();
    let allowed: bool = svc
        .check_permission(ctx.claims.is_super_admin(), &ctx.claims.role_ids, resource, action)
        .await?;

    if allowed {
        Ok(())
    } else {
        Err(WebError::from(DomainError::PermissionDenied(format!(
            "无权执行此操作: {resource}:{action}"
        ))))
    }
}
