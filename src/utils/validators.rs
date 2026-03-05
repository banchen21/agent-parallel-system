//! 数据验证器
//! 
//! 提供各种数据验证函数，包括邮箱、密码、URL等验证

use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    /// 邮箱验证正则表达式
    static ref EMAIL_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();
    
    /// 密码强度验证正则表达式
    /// 要求：至少8个字符，包含大小写字母和数字
    static ref PASSWORD_REGEX: Regex = Regex::new(
        r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d).{8,}$"
    ).unwrap();
    
    /// URL验证正则表达式
    static ref URL_REGEX: Regex = Regex::new(
        r"^https?://(?:[-\w.]|(?:%[\da-fA-F]{2}))+(?:/[-\w%!$&'()*+,;=:@.]*)*$"
    ).unwrap();
    
    /// 用户名验证正则表达式
    /// 要求：3-20个字符，只能包含字母、数字、下划线和连字符
    static ref USERNAME_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9_-]{3,20}$"
    ).unwrap();
}

/// 验证邮箱地址格式
pub fn validate_email(email: &str) -> bool {
    EMAIL_REGEX.is_match(email)
}

/// 验证密码强度
pub fn validate_password(password: &str) -> bool {
    PASSWORD_REGEX.is_match(password)
}

/// 验证URL格式
pub fn validate_url(url: &str) -> bool {
    URL_REGEX.is_match(url)
}

/// 验证用户名格式
pub fn validate_username(username: &str) -> bool {
    USERNAME_REGEX.is_match(username)
}

/// 验证任务名称
/// 要求：1-100个字符，不能为空或只包含空格
pub fn validate_task_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.len() <= 100
}

/// 验证任务描述
/// 要求：最多1000个字符
pub fn validate_task_description(description: &str) -> bool {
    description.len() <= 1000
}

/// 验证智能体名称
/// 要求：1-50个字符，不能为空或只包含空格
pub fn validate_agent_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.len() <= 50
}

/// 验证工作空间名称
/// 要求：1-50个字符，不能为空或只包含空格
pub fn validate_workspace_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.len() <= 50
}

/// 验证消息内容
/// 要求：不能为空，最多5000个字符
pub fn validate_message_content(content: &str) -> bool {
    let trimmed = content.trim();
    !trimmed.is_empty() && trimmed.len() <= 5000
}

/// 验证任务优先级
/// 要求：必须是 "low", "medium", "high" 之一
pub fn validate_task_priority(priority: &str) -> bool {
    matches!(priority.to_lowercase().as_str(), "low" | "medium" | "high")
}

/// 验证任务状态
/// 要求：必须是 "pending", "in_progress", "completed", "failed" 之一
pub fn validate_task_status(status: &str) -> bool {
    matches!(status.to_lowercase().as_str(), "pending" | "in_progress" | "completed" | "failed")
}

/// 验证智能体状态
/// 要求：必须是 "idle", "busy", "offline" 之一
pub fn validate_agent_status(status: &str) -> bool {
    matches!(status.to_lowercase().as_str(), "idle" | "busy" | "offline")
}

/// 验证整数范围
pub fn validate_integer_range(value: i32, min: i32, max: i32) -> bool {
    value >= min && value <= max
}

/// 验证浮点数范围
pub fn validate_float_range(value: f64, min: f64, max: f64) -> bool {
    value >= min && value <= max
}

/// 验证字符串长度范围
pub fn validate_string_length(value: &str, min: usize, max: usize) -> bool {
    let len = value.trim().len();
    len >= min && len <= max
}

/// 验证JSON字符串
pub fn validate_json(json_str: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json_str).is_ok()
}

/// 验证时间戳（毫秒）
pub fn validate_timestamp(timestamp: i64) -> bool {
    timestamp > 0
}

/// 验证UUID格式
pub fn validate_uuid(uuid_str: &str) -> bool {
    uuid::Uuid::parse_str(uuid_str).is_ok()
}

/// 验证API密钥格式
/// 要求：32-64个字符的十六进制字符串
pub fn validate_api_key(api_key: &str) -> bool {
    let key = api_key.trim();
    if key.len() < 32 || key.len() > 64 {
        return false;
    }
    
    // 检查是否为有效的十六进制字符串
    key.chars().all(|c| c.is_ascii_hexdigit())
}

/// 验证JWT令牌格式
pub fn validate_jwt_token(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com"));
        assert!(validate_email("user.name+tag@domain.co.uk"));
        assert!(!validate_email("invalid-email"));
        assert!(!validate_email("@domain.com"));
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("Password123"));
        assert!(validate_password("StrongPass1"));
        assert!(!validate_password("weak"));
        assert!(!validate_password("nouppercase1"));
        assert!(!validate_password("NOLOWERCASE1"));
        assert!(!validate_password("NoNumbers"));
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("user123"));
        assert!(validate_username("test-user"));
        assert!(validate_username("test_user"));
        assert!(!validate_username("ab")); // 太短
        assert!(!validate_username("verylongusernameexceedinglimit")); // 太长
        assert!(!validate_username("user@name")); // 无效字符
    }

    #[test]
    fn test_validate_task_name() {
        assert!(validate_task_name("Test Task"));
        assert!(validate_task_name("A"));
        assert!(!validate_task_name(""));
        assert!(!validate_task_name("   "));
    }

    #[test]
    fn test_validate_task_priority() {
        assert!(validate_task_priority("low"));
        assert!(validate_task_priority("MEDIUM"));
        assert!(validate_task_priority("High"));
        assert!(!validate_task_priority("invalid"));
    }

    #[test]
    fn test_validate_uuid() {
        assert!(validate_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!validate_uuid("invalid-uuid"));
    }
}