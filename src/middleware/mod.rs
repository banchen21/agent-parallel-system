pub mod auth;

// 重新导出常用类型和函数
pub use auth::{
    auth_middleware, admin_middleware, optional_auth_middleware,
    rate_limit_middleware, request_log_middleware, cors_middleware,
    AuthenticatedUser, AuthMiddleware, get_authenticated_user,
};