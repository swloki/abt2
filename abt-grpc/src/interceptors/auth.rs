use jsonwebtoken::Algorithm;
use tonic::{Request, Status};

/// 从 Authorization header 提取 Bearer token 并解码 JWT claims
fn decode_jwt_from_request<T>(request: &Request<T>) -> Result<abt::Claims, Status> {
    let token = request
        .metadata()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| Status::unauthenticated("Missing authorization token"))?;

    let config = crate::config::get_config();
    let mut validation = jsonwebtoken::Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    jsonwebtoken::decode::<abt::Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))
    .map(|data| data.claims)
}

pub fn auth_interceptor(mut request: Request<()>) -> Result<Request<()>, Status> {
    let claims = decode_jwt_from_request(&request)?;

    let auth_ctx = abt::AuthContext {
        user_id: claims.sub,
        username: claims.username,
        system_role: claims.system_role,
        dept_roles: claims.dept_roles,
        current_department_id: claims.current_department_id,
    };

    request.extensions_mut().insert(auth_ctx);
    Ok(request)
}

/// 从 gRPC request extensions 中提取 AuthContext
pub fn extract_auth<T>(request: &Request<T>) -> Result<abt::AuthContext, Status> {
    request
        .extensions()
        .get::<abt::AuthContext>()
        .cloned()
        .ok_or_else(|| Status::internal("AuthContext not found in request extensions"))
}

/// 从 request 的 Authorization header 解码 JWT 并返回 user_id
/// 用于不经 auth_interceptor 的 handler（如 AuthService）
pub fn extract_user_id_from_header<T>(request: &Request<T>) -> Result<i64, Status> {
    let claims = decode_jwt_from_request(request)?;
    Ok(claims.sub)
}
