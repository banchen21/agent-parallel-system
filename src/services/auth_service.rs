use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::{errors::AppError, security::{JwtUtil, PasswordHasherUtil}},
    models::user::{User, CreateUserRequest, LoginRequest, AuthResponse, UserResponse},
};

/// 认证服务
#[derive(Clone)]
pub struct AuthService {
    db_pool: PgPool,
}

impl AuthService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
    
    /// 用户注册
    pub async fn register(&self, request: CreateUserRequest) -> Result<UserResponse, AppError> {
        // 验证请求
        request.validate()?;
        
        // 检查用户名是否已存在
        let existing_user = sqlx::query!(
            "SELECT id FROM users WHERE username = $1 OR email = $2",
            request.username,
            request.email
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if existing_user.is_some() {
            return Err(AppError::ValidationError(
                "用户名或邮箱已存在".to_string(),
            ));
        }
        
        // 哈希密码
        let password_hash = PasswordHasherUtil::hash_password(&request.password)?;
        
        // 创建用户
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                username, email, password_hash, first_name, last_name
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
            request.username,
            request.email,
            password_hash,
            request.first_name,
            request.last_name
        )
        .fetch_one(&self.db_pool)
        .await
        .context("创建用户失败")?;
        
        Ok(user.to_response())
    }
    
    /// 用户登录
    pub async fn login(&self, request: LoginRequest) -> Result<AuthResponse, AppError> {
        // 验证请求
        request.validate()?;
        let login_id = request.username.trim();
        
        // 查找用户
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE (username = $1 OR email = $1) AND is_active = true
            "#,
            login_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let user = user.ok_or_else(|| {
            AppError::AuthenticationError("用户名或密码错误".to_string())
        })?;
        
        // 验证密码
        let is_valid = PasswordHasherUtil::verify_password(request.password.trim(), &user.password_hash)?;
        
        if !is_valid {
            return Err(AppError::AuthenticationError("用户名或密码错误".to_string()));
        }
        
        // 更新最后登录时间
        sqlx::query!(
            "UPDATE users SET last_login_at = NOW() WHERE id = $1",
            user.id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 生成令牌
        let roles = if user.role == "admin" || user.role == "super_admin" {
            vec!["admin".to_string(), "user".to_string()]
        } else {
            vec!["user".to_string()]
        };
        
        let access_token = JwtUtil::generate_access_token(user.id, &user.username, roles.clone())?;
        let refresh_token = JwtUtil::generate_refresh_token(user.id, &user.username)?;
        
        Ok(AuthResponse {
            access_token,
            refresh_token,
            token_type: "bearer".to_string(),
            expires_in: crate::core::config::CONFIG.jwt.access_token_expire_minutes * 60,
            user: user.to_response(),
        })
    }
    
    /// 刷新访问令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<AuthResponse, AppError> {
        // 验证刷新令牌
        let claims = JwtUtil::verify_token(refresh_token)?;
        
        // 检查令牌类型
        if !claims.roles.contains(&"refresh".to_string()) {
            return Err(AppError::AuthenticationError("无效的刷新令牌".to_string()));
        }
        
        // 查找用户
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1 AND is_active = true
            "#,
            claims.sub
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let user = user.ok_or_else(|| {
            AppError::AuthenticationError("用户不存在或已被禁用".to_string())
        })?;
        
        // 生成新的访问令牌
        let roles = if user.is_superuser {
            vec!["admin".to_string(), "user".to_string()]
        } else {
            vec!["user".to_string()]
        };
        
        let access_token = JwtUtil::generate_access_token(user.id, &user.username, roles)?;
        let new_refresh_token = JwtUtil::generate_refresh_token(user.id, &user.username)?;
        
        Ok(AuthResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "bearer".to_string(),
            expires_in: crate::core::config::CONFIG.jwt.access_token_expire_minutes * 60,
            user: user.to_response(),
        })
    }
    
    /// 获取当前用户信息
    pub async fn get_current_user(&self, user_id: Uuid) -> Result<UserResponse, AppError> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1 AND is_active = true
            "#,
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let user = user.ok_or_else(|| {
            AppError::NotFound("用户不存在".to_string())
        })?;
        
        Ok(user.to_response())
    }
    
    /// 验证用户令牌
    pub async fn validate_token(&self, token: &str) -> Result<UserResponse, AppError> {
        let claims = JwtUtil::verify_token(token)?;
        
        // 检查令牌类型
        if claims.roles.contains(&"refresh".to_string()) {
            return Err(AppError::AuthenticationError("请使用访问令牌".to_string()));
        }
        
        self.get_current_user(claims.sub).await
    }
    
    /// 用户登出
    pub async fn logout(&self, _user_id: Uuid) -> Result<(), AppError> {
        // 在实际应用中，这里可以将令牌加入黑名单
        // 目前我们使用短期的JWT，所以直接返回成功
        Ok(())
    }
    
    /// 更改密码
    pub async fn change_password(
        &self,
        user_id: Uuid,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        // 验证新密码
        crate::core::security::InputValidator::validate_password(new_password)?;
        
        // 获取用户
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1 AND is_active = true
            "#,
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let user = user.ok_or_else(|| {
            AppError::NotFound("用户不存在".to_string())
        })?;
        
        // 验证当前密码
        let is_valid = PasswordHasherUtil::verify_password(current_password, &user.password_hash)?;
        
        if !is_valid {
            return Err(AppError::AuthenticationError("当前密码错误".to_string()));
        }
        
        // 哈希新密码
        let new_password_hash = PasswordHasherUtil::hash_password(new_password)?;
        
        // 更新密码
        sqlx::query!(
            "UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2",
            new_password_hash,
            user_id
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
    
    /// 重置密码（管理员功能）
    pub async fn reset_password(
        &self,
        admin_user_id: Uuid,
        target_user_id: Uuid,
        new_password: &str,
    ) -> Result<(), AppError> {
        // 验证新密码
        crate::core::security::InputValidator::validate_password(new_password)?;
        
        // 检查管理员权限
        let admin_user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1 AND is_active = true AND is_superuser = true
            "#,
            admin_user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if admin_user.is_none() {
            return Err(AppError::PermissionDenied("需要管理员权限".to_string()));
        }
        
        // 检查目标用户
        let target_user = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1
            "#,
            target_user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if target_user.is_none() {
            return Err(AppError::NotFound("目标用户不存在".to_string()));
        }
        
        // 哈希新密码
        let new_password_hash = PasswordHasherUtil::hash_password(new_password)?;
        
        // 更新密码
        sqlx::query!(
            "UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2",
            new_password_hash,
            target_user_id
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
}
