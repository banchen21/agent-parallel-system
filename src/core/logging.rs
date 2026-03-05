use std::env;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

use super::config;

/// 初始化日志系统
pub fn init_logging() {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| config::CONFIG.logging.level.clone());
    let log_format = config::CONFIG.logging.format.clone();
    
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&log_level));
    
    match log_format.as_str() {
        "json" => init_json_logging(env_filter),
        "pretty" => init_pretty_logging(env_filter),
        _ => init_default_logging(env_filter),
    }
    
    tracing::info!(
        "Logging initialized with level: {}, format: {}",
        log_level,
        log_format
    );
}

/// 初始化JSON格式日志
fn init_json_logging(env_filter: EnvFilter) {
    let json_layer = fmt::layer()
        .json()
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_current_span(true);
    
    Registry::default()
        .with(env_filter)
        .with(json_layer)
        .init();
}

/// 初始化美化格式日志
fn init_pretty_logging(env_filter: EnvFilter) {
    let pretty_layer = fmt::layer()
        .pretty()
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);
    
    Registry::default()
        .with(env_filter)
        .with(pretty_layer)
        .init();
}

/// 初始化默认格式日志
fn init_default_logging(env_filter: EnvFilter) {
    let default_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_target(true);
    
    Registry::default()
        .with(env_filter)
        .with(default_layer)
        .init();
}

/// 创建结构化日志事件
#[macro_export]
macro_rules! structured_log {
    ($level:expr, $message:expr, $($key:ident = $value:expr),*) => {{
        match $level {
            tracing::Level::ERROR => {
                tracing::error!(
                    $message,
                    $($key = $value),*
                )
            }
            tracing::Level::WARN => {
                tracing::warn!(
                    $message,
                    $($key = $value),*
                )
            }
            tracing::Level::INFO => {
                tracing::info!(
                    $message,
                    $($key = $value),*
                )
            }
            tracing::Level::DEBUG => {
                tracing::debug!(
                    $message,
                    $($key = $value),*
                )
            }
            tracing::Level::TRACE => {
                tracing::trace!(
                    $message,
                    $($key = $value),*
                )
            }
        }
    }};
}

/// 记录API请求日志
pub fn log_api_request(
    method: &str,
    path: &str,
    status_code: u16,
    duration_ms: u64,
    user_id: Option<String>,
    ip: Option<String>,
) {
    structured_log!(
        Level::INFO,
        "API请求: method={} path={} status={} duration_ms={} user_id={} ip={}",
        method = method,
        path = path,
        status = status_code,
        duration_ms = duration_ms,
        user_id = user_id.unwrap_or_else(|| "anonymous".to_string()),
        ip = ip.unwrap_or_else(|| "unknown".to_string())
    );
}

/// 记录任务执行日志
pub fn log_task_execution(
    task_id: &str,
    action: &str,
    status: &str,
    duration_ms: Option<u64>,
    error: Option<&str>,
) {
    if let Some(err) = error {
        structured_log!(
            Level::ERROR,
            "任务执行失败: task_id={} action={} status={} duration_ms={} error={}",
            task_id = task_id,
            action = action,
            status = status,
            duration_ms = duration_ms.unwrap_or(0),
            error = err
        );
    } else {
        structured_log!(
            Level::INFO,
            "任务执行: task_id={} action={} status={} duration_ms={}",
            task_id = task_id,
            action = action,
            status = status,
            duration_ms = duration_ms.unwrap_or(0)
        );
    }
}

/// 记录智能体活动日志
pub fn log_agent_activity(
    agent_id: &str,
    action: &str,
    task_id: Option<&str>,
    details: Option<&str>,
) {
    structured_log!(
        Level::INFO,
        "智能体活动: agent_id={} action={} task_id={} details={}",
        agent_id = agent_id,
        action = action,
        task_id = task_id.unwrap_or("none"),
        details = details.unwrap_or("")
    );
}

/// 记录数据库查询日志
pub fn log_database_query(
    query: &str,
    duration_ms: u64,
    rows_affected: Option<u64>,
    error: Option<&str>,
) {
    if let Some(err) = error {
        structured_log!(
            Level::ERROR,
            "数据库查询失败: query={} duration_ms={} error={}",
            query = query,
            duration_ms = duration_ms,
            error = err
        );
    } else {
        structured_log!(
            Level::DEBUG,
            "数据库查询: query={} duration_ms={} rows_affected={}",
            query = query,
            duration_ms = duration_ms,
            rows_affected = rows_affected.unwrap_or(0)
        );
    }
}
