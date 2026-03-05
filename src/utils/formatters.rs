//! 数据格式化器
//! 
//! 提供各种数据格式化函数，包括时间、JSON、响应等格式化

use chrono::{DateTime, Utc};
use serde_json::Value;

/// 格式化时间戳为可读字符串
pub fn format_timestamp(timestamp: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp(timestamp / 1000, 0);
    match dt {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => "Invalid timestamp".to_string(),
    }
}

/// 格式化持续时间（毫秒）为可读字符串
pub fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1000 {
        format!("{}ms", duration_ms)
    } else if duration_ms < 60000 {
        format!("{:.2}s", duration_ms as f64 / 1000.0)
    } else if duration_ms < 3600000 {
        let minutes = duration_ms / 60000;
        let seconds = (duration_ms % 60000) / 1000;
        format!("{}m {}s", minutes, seconds)
    } else {
        let hours = duration_ms / 3600000;
        let minutes = (duration_ms % 3600000) / 60000;
        format!("{}h {}m", hours, minutes)
    }
}

/// 格式化文件大小为可读字符串
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let exponent = (bytes as f64).log(1024.0).floor() as u32;
    let size = bytes as f64 / 1024_f64.powi(exponent as i32);
    let unit = UNITS[exponent as usize];
    
    if exponent == 0 {
        format!("{} {}", bytes, unit)
    } else {
        format!("{:.2} {}", size, unit)
    }
}

/// 格式化JSON为美化字符串
pub fn format_json_pretty(json: &Value) -> String {
    serde_json::to_string_pretty(json).unwrap_or_else(|_| "Invalid JSON".to_string())
}

/// 格式化JSON为压缩字符串
pub fn format_json_compact(json: &Value) -> String {
    serde_json::to_string(json).unwrap_or_else(|_| "Invalid JSON".to_string())
}

/// 格式化错误消息
pub fn format_error_message(error: &str) -> String {
    // 将错误消息转换为更友好的格式
    let error = error.trim();
    if error.is_empty() {
        "Unknown error".to_string()
    } else {
        // 首字母大写
        let mut chars = error.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}

/// 格式化任务状态为可读字符串
pub fn format_task_status(status: &str) -> String {
    match status {
        "pending" => "待处理".to_string(),
        "in_progress" => "进行中".to_string(),
        "completed" => "已完成".to_string(),
        "failed" => "失败".to_string(),
        _ => status.to_string(),
    }
}

/// 格式化任务优先级为可读字符串
pub fn format_task_priority(priority: &str) -> String {
    match priority {
        "low" => "低".to_string(),
        "medium" => "中".to_string(),
        "high" => "高".to_string(),
        _ => priority.to_string(),
    }
}

/// 格式化智能体状态为可读字符串
pub fn format_agent_status(status: &str) -> String {
    match status {
        "idle" => "空闲".to_string(),
        "busy" => "忙碌".to_string(),
        "offline" => "离线".to_string(),
        _ => status.to_string(),
    }
}

/// 格式化百分比
pub fn format_percentage(value: f64, total: f64) -> String {
    if total == 0.0 {
        "0%".to_string()
    } else {
        let percentage = (value / total) * 100.0;
        format!("{:.1}%", percentage)
    }
}

/// 格式化数字为带千位分隔符的字符串
pub fn format_number_with_commas(number: i64) -> String {
    let num_str = number.to_string();
    let mut result = String::new();
    let mut count = 0;
    
    for c in num_str.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }
    
    result.chars().rev().collect()
}

/// 格式化布尔值为中文
pub fn format_boolean(value: bool) -> String {
    if value {
        "是".to_string()
    } else {
        "否".to_string()
    }
}

/// 格式化API响应
pub fn format_api_response<T: serde::Serialize>(data: T, message: &str) -> serde_json::Value {
    serde_json::json!({
        "success": true,
        "message": message,
        "data": data,
        "timestamp": Utc::now().timestamp_millis()
    })
}

/// 格式化API错误响应
pub fn format_api_error(message: &str, error_code: &str) -> serde_json::Value {
    serde_json::json!({
        "success": false,
        "message": message,
        "error_code": error_code,
        "timestamp": Utc::now().timestamp_millis()
    })
}

/// 格式化日志消息
pub fn format_log_message(level: &str, message: &str, context: Option<&str>) -> String {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
    match context {
        Some(ctx) => format!("[{}] [{}] {} - {}", timestamp, level, ctx, message),
        None => format!("[{}] [{}] {}", timestamp, level, message),
    }
}

/// 格式化用户显示名称
pub fn format_user_display_name(username: &str, email: &str) -> String {
    if !username.is_empty() {
        username.to_string()
    } else {
        // 从邮箱提取用户名部分
        email.split('@').next().unwrap_or(email).to_string()
    }
}

/// 格式化工作空间路径
pub fn format_workspace_path(workspace_name: &str) -> String {
    workspace_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// 格式化任务进度
pub fn format_task_progress(completed: i32, total: i32) -> String {
    if total == 0 {
        "0%".to_string()
    } else {
        let progress = (completed as f64 / total as f64) * 100.0;
        format!("{:.1}% ({}/{})", progress, completed, total)
    }
}

/// 格式化智能体能力描述
pub fn format_agent_capabilities(capabilities: &[String]) -> String {
    if capabilities.is_empty() {
        "无特殊能力".to_string()
    } else {
        capabilities.join(", ")
    }
}

/// 格式化消息类型
pub fn format_message_type(message_type: &str) -> String {
    match message_type {
        "task_assigned" => "任务分配".to_string(),
        "task_completed" => "任务完成".to_string(),
        "task_failed" => "任务失败".to_string(),
        "agent_status_changed" => "智能体状态变更".to_string(),
        "system_notification" => "系统通知".to_string(),
        _ => message_type.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_timestamp() {
        let timestamp = 1672531200000; // 2023-01-01 00:00:00 UTC
        assert_eq!(format_timestamp(timestamp), "2023-01-01 00:00:00");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.50s");
        assert_eq!(format_duration(65000), "1m 5s");
        assert_eq!(format_duration(3665000), "1h 1m");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1048576), "1.00 MB");
        assert_eq!(format_file_size(500), "500 B");
    }

    #[test]
    fn test_format_task_status() {
        assert_eq!(format_task_status("pending"), "待处理");
        assert_eq!(format_task_status("in_progress"), "进行中");
        assert_eq!(format_task_status("completed"), "已完成");
        assert_eq!(format_task_status("unknown"), "unknown");
    }

    #[test]
    fn test_format_number_with_commas() {
        assert_eq!(format_number_with_commas(1000), "1,000");
        assert_eq!(format_number_with_commas(1000000), "1,000,000");
        assert_eq!(format_number_with_commas(123), "123");
    }

    #[test]
    fn test_format_api_response() {
        let data = json!({"id": 1, "name": "test"});
        let response = format_api_response(data, "Success");
        assert!(response["success"].as_bool().unwrap());
        assert_eq!(response["message"].as_str().unwrap(), "Success");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(25.0, 100.0), "25.0%");
        assert_eq!(format_percentage(0.0, 0.0), "0%");
        assert_eq!(format_percentage(33.333, 100.0), "33.3%");
    }
}