use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

use crate::{
    core::{config, errors::AppError, security::Claims},
    AppState,
};

/// 认证用户信息
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
    pub roles: Vec<String>,
}

/// 认证中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 从请求头中提取令牌
    let token = extract_token_from_request(&request)?;
    
    // 验证令牌
    let claims = validate_token(&token)?;
    
    // 将用户信息添加到请求扩展中
    let authenticated_user = AuthenticatedUser {
        id: claims.sub,
        username: claims.username,
        roles: claims.roles,
    };
    
    request.extensions_mut().insert(authenticated_user);
    
    // 继续处理请求
    Ok(next.run(request).await)
}

/// 管理员中间件
pub async fn admin_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 先进行普通认证
    let mut request = request;
    let token = extract_token_from_request(&request)?;
    let claims = validate_token(&token)?;
    
    // 检查是否为管理员
    if !claims.roles.contains(&"admin".to_string()) {
        return Err(AppError::PermissionDenied("需要管理员权限".to_string()));
    }
    
    // 将用户信息添加到请求扩展中
    let authenticated_user = AuthenticatedUser {
        id: claims.sub,
        username: claims.username,
        roles: claims.roles,
    };
    
    request.extensions_mut().insert(authenticated_user);
    
    // 继续处理请求
    Ok(next.run(request).await)
}

/// 从请求中提取令牌
fn extract_token_from_request(request: &Request) -> Result<String, AppError> {
    // 从Authorization头中提取
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    
    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            header.trim_start_matches("Bearer ").to_string()
        }
        _ => {
            // 尝试从查询参数中获取
            let query = request.uri().query().unwrap_or("");
            let params: Vec<&str> = query.split('&').collect();
            
            for param in params {
                if param.starts_with("token=") {
                    return Ok(param.trim_start_matches("token=").to_string());
                }
            }
            
            return Err(AppError::AuthenticationError("未提供认证令牌".to_string()));
        }
    };
    
    if token.is_empty() {
        return Err(AppError::AuthenticationError("认证令牌为空".to_string()));
    }
    
    Ok(token)
}

/// 验证JWT令牌
fn validate_token(token: &str) -> Result<Claims, AppError> {
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config::CONFIG.jwt.secret.as_bytes()),
        &validation,
    )
    .map_err(|e| AppError::AuthenticationError(e.to_string()))?;
    
    // 检查令牌是否过期
    let now = chrono::Utc::now().timestamp();
    if token_data.claims.exp < now {
        return Err(AppError::AuthenticationError("令牌已过期".to_string()));
    }
    
    Ok(token_data.claims)
}

/// 从请求扩展中获取认证用户
pub fn get_authenticated_user(request: &Request) -> Result<AuthenticatedUser, AppError> {
    request
        .extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::AuthenticationError("用户未认证".to_string()))
}

/// 可选认证中间件（不强制要求认证）
pub async fn optional_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // 尝试提取令牌
    if let Ok(token) = extract_token_from_request(&request) {
        if let Ok(claims) = validate_token(&token) {
            // 将用户信息添加到请求扩展中
            let authenticated_user = AuthenticatedUser {
                id: claims.sub,
                username: claims.username,
                roles: claims.roles,
            };
            request.extensions_mut().insert(authenticated_user);
        }
    }
    
    // 继续处理请求
    next.run(request).await
}

/// 速率限制中间件
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 获取用户信息（如果存在）
    if let Ok(user) = get_authenticated_user(&request) {
        let endpoint = request.uri().path().to_string();
        
        // 检查速率限制
        let allowed = crate::core::security::RateLimiter::check_user_rate_limit(
            &state.redis_pool,
            user.id,
            &endpoint,
        )
        .await?;
        
        if !allowed {
            return Err(AppError::RateLimitExceeded);
        }
    }
    
    // 继续处理请求
    Ok(next.run(request).await)
}

/// CORS中间件（简化版）
pub fn cors_middleware() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, CorsLayer};
    
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_credentials(false)
}

/// 请求日志中间件
pub async fn request_log_middleware(
    request: Request,
    next: Next,
) -> Response {
    use std::time::Instant;
    
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();
    
    // 处理请求
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    let status = response.status().as_u16();
    
    // 记录请求日志
    crate::core::logging::log_api_request(
        &method,
        &path,
        status,
        duration.as_millis() as u64,
        None, // 这里可以获取用户ID
        None, // 这里可以获取IP地址
    );
    
    response
}