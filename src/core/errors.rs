use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// 应用错误类型
#[derive(Debug, Error)]
pub enum AppError {
    /// 验证错误
    #[error("验证错误: {0}")]
    ValidationError(String),
    
    /// 未找到资源
    #[error("未找到资源: {0}")]
    NotFound(String),
    
    /// 权限拒绝
    #[error("权限拒绝: {0}")]
    PermissionDenied(String),
    
    /// 认证失败
    #[error("认证失败: {0}")]
    AuthenticationError(String),
    
    /// 数据库错误
    #[error("数据库错误: {0}")]
    DatabaseError(String),
    
    /// Redis错误
    #[error("Redis错误: {0}")]
    RedisError(String),
    
    /// JSON序列化错误
    #[error("JSON序列化错误: {0}")]
    JsonError(String),
    
    /// 序列化错误
    #[error("序列化错误: {0}")]
    SerializationError(String),
    
    /// 内部服务器错误
    #[error("内部服务器错误")]
    InternalServerError,
    
    /// 内部错误（带明细）
    #[error("内部错误: {0}")]
    InternalError(String),
    
    /// 限流错误
    #[error("请求频率超限")]
    RateLimitExceeded,
    
    /// 外部API错误
    #[error("外部API错误: {0}")]
    ExternalApiError(String),
    
    /// 任务执行错误
    #[error("任务执行错误: {0}")]
    TaskExecutionError(String),
    
    /// 智能体错误
    #[error("智能体错误: {0}")]
    AgentError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, error_message) = match self {
            AppError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                msg,
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                msg,
            ),
            AppError::PermissionDenied(msg) => (
                StatusCode::FORBIDDEN,
                "PERMISSION_DENIED",
                msg,
            ),
            AppError::AuthenticationError(msg) => (
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_FAILED",
                msg,
            ),
            AppError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "数据库操作失败".to_string(),
            ),
            AppError::RedisError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "REDIS_ERROR",
                "缓存操作失败".to_string(),
            ),
            AppError::JsonError(_) => (
                StatusCode::BAD_REQUEST,
                "JSON_ERROR",
                "JSON序列化失败".to_string(),
            ),
            AppError::SerializationError(msg) => (
                StatusCode::BAD_REQUEST,
                "SERIALIZATION_ERROR",
                msg,
            ),
            AppError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_SERVER_ERROR",
                "内部服务器错误".to_string(),
            ),
            AppError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg,
            ),
            AppError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED",
                "请求频率超限".to_string(),
            ),
            AppError::ExternalApiError(msg) => (
                StatusCode::BAD_GATEWAY,
                "EXTERNAL_API_ERROR",
                msg,
            ),
            AppError::TaskExecutionError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_EXECUTION_ERROR",
                msg,
            ),
            AppError::AgentError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AGENT_ERROR",
                msg,
            ),
        };

        let body = Json(json!({
            "success": false,
            "error": {
                "code": error_code,
                "message": error_message,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));

        (status, body).into_response()
    }
}

/// 从anyhow::Error转换
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // 根据错误类型进行更精确的转换
        if let Some(db_err) = err.downcast_ref::<sqlx::Error>() {
            match db_err {
                sqlx::Error::RowNotFound => AppError::NotFound("资源不存在".to_string()),
                _ => AppError::DatabaseError(db_err.to_string()),
            }
        } else if let Some(redis_err) = err.downcast_ref::<redis::RedisError>() {
            AppError::RedisError(redis_err.to_string())
        } else if let Some(json_err) = err.downcast_ref::<serde_json::Error>() {
            AppError::JsonError(json_err.to_string())
        } else {
            AppError::InternalError(err.to_string())
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        AppError::DatabaseError(value.to_string())
    }
}

impl From<redis::RedisError> for AppError {
    fn from(value: redis::RedisError) -> Self {
        AppError::RedisError(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::JsonError(value.to_string())
    }
}

impl From<bb8::RunError<redis::RedisError>> for AppError {
    fn from(value: bb8::RunError<redis::RedisError>) -> Self {
        AppError::RedisError(value.to_string())
    }
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, AppError>;
