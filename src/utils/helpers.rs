use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 生成唯一的ID
pub fn generate_id() -> Uuid {
    Uuid::new_v4()
}

/// 格式化时间戳
pub fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

/// 解析时间戳
pub fn parse_timestamp(timestamp_str: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(timestamp_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// 计算时间差（秒）
pub fn time_diff_seconds(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    (end - start).num_seconds()
}

/// 生成随机字符串
pub fn generate_random_string(length: usize) -> String {
    use rand::Rng;
    
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    
    let mut rng = rand::thread_rng();
    let random_string: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    
    random_string
}

/// 计算字符串的SHA256哈希
pub fn sha256_hash(input: &str) -> String {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    
    hex::encode(result)
}

/// 检查字符串是否为有效的UUID
pub fn is_valid_uuid(uuid_str: &str) -> bool {
    Uuid::parse_str(uuid_str).is_ok()
}

/// 安全的字符串截断
pub fn truncate_string(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        format!("{}...", &s[..max_length.saturating_sub(3)])
    }
}

/// 将字节转换为人类可读的大小
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let base = 1024_f64;
    let exponent = (bytes as f64).log(base).floor() as u32;
    let exponent = exponent.min((UNITS.len() - 1) as u32);
    
    let size = bytes as f64 / base.powi(exponent as i32);
    
    format!("{:.2} {}", size, UNITS[exponent as usize])
}

/// 生成任务进度条
pub fn generate_progress_bar(progress: i32, width: usize) -> String {
    let filled = (progress as f32 / 100.0 * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// 计算任务预估完成时间
pub fn estimate_completion_time(
    start_time: DateTime<Utc>,
    progress: i32,
) -> Option<DateTime<Utc>> {
    if progress <= 0 || progress >= 100 {
        return None;
    }
    
    let elapsed = Utc::now() - start_time;
    let total_estimated = elapsed * 100 / progress;
    
    Some(start_time + total_estimated)
}

/// 验证JSON字符串
pub fn is_valid_json(json_str: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json_str).is_ok()
}

/// 深度合并两个JSON对象
pub fn deep_merge_json(
    base: serde_json::Value,
    overlay: serde_json::Value,
) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(ref mut base_obj), serde_json::Value::Object(overlay_obj)) => {
            for (key, value) in overlay_obj {
                if let Some(existing_value) = base_obj.get_mut(&key) {
                    *existing_value = deep_merge_json(existing_value.clone(), value);
                } else {
                    base_obj.insert(key, value);
                }
            }
            serde_json::Value::Object(base_obj.clone())
        }
        (_, overlay) => overlay,
    }
}

/// 生成任务优先级颜色
pub fn get_priority_color(priority: &str) -> &'static str {
    match priority.to_lowercase().as_str() {
        "urgent" => "#ff4444",
        "high" => "#ffaa00",
        "medium" => "#00aaff",
        "low" => "#44aa44",
        _ => "#666666",
    }
}

/// 生成智能体状态颜色
pub fn get_agent_status_color(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "online" => "#44aa44",
        "busy" => "#ffaa00",
        "idle" => "#00aaff",
        "offline" => "#666666",
        "error" => "#ff4444",
        _ => "#666666",
    }
}

/// 计算智能体负载百分比
pub fn calculate_agent_load_percentage(current_load: i32, max_concurrent_tasks: i32) -> f64 {
    if max_concurrent_tasks == 0 {
        return 0.0;
    }
    
    (current_load as f64 / max_concurrent_tasks as f64 * 100.0).min(100.0)
}

/// 生成任务状态图标
pub fn get_task_status_icon(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "pending" => "⏳",
        "in_progress" => "🔄",
        "completed" => "✅",
        "failed" => "❌",
        "cancelled" => "🚫",
        _ => "❓",
    }
}

/// 生成智能体能力图标
pub fn get_capability_icon(capability: &str) -> &'static str {
    match capability.to_lowercase().as_str() {
        "data_analysis" => "📊",
        "report_writing" => "📝",
        "code_generation" => "💻",
        "translation" => "🌐",
        "summarization" => "📄",
        "content_writing" => "✍️",
        "research" => "🔍",
        "calculation" => "🧮",
        "classification" => "🏷️",
        "data_collection" => "📥",
        "data_processing" => "⚙️",
        "general_processing" => "🔄",
        _ => "🔧",
    }
}