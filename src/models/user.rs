use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::core::security::InputValidator;

/// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Validate)]
pub struct User {
    pub id: Uuid,
    
    #[validate(length(min = 3, max = 50))]
    pub username: String,
    
    #[validate(email)]
    pub email: String,
    
    pub password_hash: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: String,
    pub is_superuser: bool,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 用户创建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 3, max = 50))]
    pub username: String,
    
    #[validate(email)]
    pub email: String,
    
    #[validate(length(min = 8))]
    pub password: String,
    
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

/// 用户登录请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 用户响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_active: bool,
    pub is_superuser: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 认证响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

/// 用户角色
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    User,
    Admin,
    Moderator,
}

impl User {
    /// 转换为响应格式
    pub fn to_response(&self) -> UserResponse {
        let full_name = match (&self.first_name, &self.last_name) {
            (Some(first), Some(last)) => Some(format!("{} {}", first, last)),
            (Some(first), None) => Some(first.clone()),
            (None, Some(last)) => Some(last.clone()),
            (None, None) => None,
        };

        UserResponse {
            id: self.id,
            username: self.username.clone(),
            email: self.email.clone(),
            full_name,
            avatar_url: None,
            is_active: self.is_active,
            is_superuser: self.is_superuser || self.role.eq_ignore_ascii_case("admin"),
            last_login: self.last_login_at,
            created_at: self.created_at,
        }
    }
    
    /// 验证用户数据
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        InputValidator::validate_username(&self.username)?;
        InputValidator::validate_email(&self.email)?;
        Ok(())
    }
}

impl CreateUserRequest {
    /// 验证创建用户请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        InputValidator::validate_username(&self.username)?;
        InputValidator::validate_email(&self.email)?;
        InputValidator::validate_password(&self.password)?;
        Ok(())
    }
}

impl LoginRequest {
    /// 验证登录请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        if self.username.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "用户名不能为空".to_string(),
            ));
        }
        if self.password.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "密码不能为空".to_string(),
            ));
        }
        Ok(())
    }
}
