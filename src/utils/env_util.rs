// 辅助函数：从环境变量读取并解析为指定类型，若失败则返回默认值
pub fn env_var_or_default<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
