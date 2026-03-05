use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{config, errors::AppError};

/// JWT声明
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,        // 用户ID
    pub username: String, // 用户名
    pub roles: Vec<String>, // 用户角色
    #[serde(default)]
    pub jti: Option<String>, // 令牌唯一ID（兼容旧令牌可为空）
    pub exp: i64,         // 过期时间
    pub iat: i64,         // 签发时间
}

/// 密码哈希和验证
pub struct PasswordHasherUtil;

impl PasswordHasherUtil {
    /// 哈希密码
    pub fn hash_password(password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::InternalServerError)?
            .to_string();
        
        Ok(password_hash)
    }
    
    /// 验证密码
    pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::InternalServerError)?;
        
        let result = Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();
        
        Ok(result)
    }
}

/// JWT工具
pub struct JwtUtil;

impl JwtUtil {
    /// 生成访问令牌
    pub fn generate_access_token(
        user_id: Uuid,
        username: &str,
        roles: Vec<String>,
    ) -> Result<String, AppError> {
        let now = chrono::Utc::now();
        let expire = now + chrono::Duration::minutes(
            config::CONFIG.jwt.access_token_expire_minutes,
        );
        
        let claims = Claims {
            sub: user_id,
            username: username.to_string(),
            roles,
            jti: Some(Uuid::new_v4().to_string()),
            exp: expire.timestamp(),
            iat: now.timestamp(),
        };
        
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(config::CONFIG.jwt.secret.as_bytes()),
        )
        .map_err(|e| AppError::AuthenticationError(e.to_string()))?;
        
        Ok(token)
    }
    
    /// 生成刷新令牌
    pub fn generate_refresh_token(user_id: Uuid, username: &str) -> Result<String, AppError> {
        let now = chrono::Utc::now();
        let expire = now + chrono::Duration::days(
            config::CONFIG.jwt.refresh_token_expire_days,
        );
        
        let claims = Claims {
            sub: user_id,
            username: username.to_string(),
            roles: vec!["refresh".to_string()],
            jti: Some(Uuid::new_v4().to_string()),
            exp: expire.timestamp(),
            iat: now.timestamp(),
        };
        
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(config::CONFIG.jwt.secret.as_bytes()),
        )
        .map_err(|e| AppError::AuthenticationError(e.to_string()))?;
        
        Ok(token)
    }
    
    /// 验证JWT令牌
    pub fn verify_token(token: &str) -> Result<Claims, AppError> {
        let validation = Validation::new(Algorithm::HS256);
        
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(config::CONFIG.jwt.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| AppError::AuthenticationError(e.to_string()))?;
        
        Ok(token_data.claims)
    }
    
    /// 从令牌中提取用户ID
    pub fn extract_user_id_from_token(token: &str) -> Result<Uuid, AppError> {
        let claims = Self::verify_token(token)?;
        Ok(claims.sub)
    }
}

/// 输入验证工具
pub struct InputValidator;

impl InputValidator {
    /// 验证邮箱格式
    pub fn validate_email(email: &str) -> Result<(), AppError> {
        if email.is_empty() {
            return Err(AppError::ValidationError("邮箱不能为空".to_string()));
        }
        
        let email_regex = regex::Regex::new(
            r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$",
        )
        .unwrap();
        
        if !email_regex.is_match(email) {
            return Err(AppError::ValidationError("邮箱格式不正确".to_string()));
        }
        
        Ok(())
    }
    
    /// 验证用户名
    pub fn validate_username(username: &str) -> Result<(), AppError> {
        if username.is_empty() {
            return Err(AppError::ValidationError("用户名不能为空".to_string()));
        }
        
        if username.len() < 3 {
            return Err(AppError::ValidationError("用户名长度不能少于3个字符".to_string()));
        }
        
        if username.len() > 50 {
            return Err(AppError::ValidationError("用户名长度不能超过50个字符".to_string()));
        }

        // 兼容邮箱作为登录名场景
        if username.contains('@') {
            return Self::validate_email(username);
        }

        // 允许常见用户名字符：字母、数字、下划线、点、短横线
        let username_regex = regex::Regex::new(r"^[a-zA-Z0-9_.-]+$").unwrap();
        if !username_regex.is_match(username) {
            return Err(AppError::ValidationError(
                "用户名只能包含字母、数字、下划线、点或短横线，或使用邮箱地址".to_string(),
            ));
        }
        
        Ok(())
    }
    
    /// 验证密码强度
    pub fn validate_password(password: &str) -> Result<(), AppError> {
        if password.is_empty() {
            return Err(AppError::ValidationError("密码不能为空".to_string()));
        }
        
        if password.len() < 8 {
            return Err(AppError::ValidationError("密码长度不能少于8个字符".to_string()));
        }
        
        if !password.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(AppError::ValidationError(
                "密码必须包含至少一个大写字母".to_string(),
            ));
        }
        
        if !password.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(AppError::ValidationError(
                "密码必须包含至少一个小写字母".to_string(),
            ));
        }
        
        if !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(AppError::ValidationError("密码必须包含至少一个数字".to_string()));
        }
        
        Ok(())
    }
    
    /// 验证任务标题
    pub fn validate_task_title(title: &str) -> Result<(), AppError> {
        if title.trim().is_empty() {
            return Err(AppError::ValidationError("任务标题不能为空".to_string()));
        }
        
        if title.len() > 500 {
            return Err(AppError::ValidationError("任务标题长度不能超过500字符".to_string()));
        }
        
        Ok(())
    }
}

/// 权限检查工具
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查用户是否有权限访问工作空间
    pub fn can_access_workspace(
        user_roles: &[String],
        workspace_permissions: &[String],
    ) -> bool {
        // 管理员可以访问所有工作空间
        if user_roles.contains(&"admin".to_string()) {
            return true;
        }
        
        // 检查用户角色是否在工作空间权限中
        user_roles
            .iter()
            .any(|role| workspace_permissions.contains(role))
    }
    
    /// 检查用户是否有权限执行操作
    pub fn can_perform_action(user_roles: &[String], required_roles: &[String]) -> bool {
        user_roles
            .iter()
            .any(|role| required_roles.contains(role))
    }
    
    /// 检查智能体是否有权限访问工具
    pub fn can_agent_access_tool(agent_capabilities: &[String], required_capabilities: &[String]) -> bool {
        required_capabilities
            .iter()
            .all(|capability| agent_capabilities.contains(capability))
    }
}

/// 速率限制工具
pub struct RateLimiter;

impl RateLimiter {
    /// 检查API调用频率
    pub async fn check_rate_limit(
        redis_pool: &bb8::Pool<bb8_redis::RedisConnectionManager>,
        key: &str,
        limit: u32,
        window_seconds: u64,
    ) -> Result<bool, AppError> {
        let mut conn = redis_pool.get().await?;
        
        let current = redis::cmd("INCR")
            .arg(key)
            .query_async::<u32>(&mut *conn)
            .await?;
        
        if current == 1 {
            // 第一次调用，设置过期时间
            redis::cmd("EXPIRE")
                .arg(key)
                .arg(window_seconds)
                .query_async::<bool>(&mut *conn)
                .await?;
        }
        
        Ok(current <= limit)
    }
    
    /// 检查用户API调用频率
    pub async fn check_user_rate_limit(
        redis_pool: &bb8::Pool<bb8_redis::RedisConnectionManager>,
        user_id: Uuid,
        endpoint: &str,
    ) -> Result<bool, AppError> {
        let key = format!("rate_limit:{}:{}", user_id, endpoint);
        Self::check_rate_limit(
            redis_pool,
            &key,
            config::CONFIG.security.rate_limit,
            config::CONFIG.security.rate_limit_window,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::JwtUtil;
    use uuid::Uuid;

    #[test]
    fn access_tokens_should_be_unique_even_when_issued_immediately() {
        let user_id = Uuid::new_v4();
        let roles = vec!["user".to_string()];

        let token1 =
            JwtUtil::generate_access_token(user_id, "token-user", roles.clone()).unwrap();
        let token2 = JwtUtil::generate_access_token(user_id, "token-user", roles).unwrap();

        assert_ne!(token1, token2);
    }

    #[test]
    fn refresh_tokens_should_be_unique_even_when_issued_immediately() {
        let user_id = Uuid::new_v4();

        let token1 = JwtUtil::generate_refresh_token(user_id, "token-user").unwrap();
        let token2 = JwtUtil::generate_refresh_token(user_id, "token-user").unwrap();

        assert_ne!(token1, token2);
    }
}
