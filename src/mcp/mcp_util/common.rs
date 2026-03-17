use serde_json::Value;
use std::path::{Component, PathBuf};

use crate::mcp::model::McpError;
use crate::utils::workspace_path::workspace_dir;

pub fn arg_string(arguments: &Value, key: &str) -> Option<String> {
    arguments.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

pub fn arg_u64(arguments: &Value, key: &str) -> Option<u64> {
    arguments.get(key).and_then(Value::as_u64)
}

pub fn arg_bool(arguments: &Value, key: &str, default: bool) -> bool {
    arguments
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub fn workspace_root_from_args(arguments: &Value) -> PathBuf {
    if let Some(workspace_name) = arg_string(arguments, "workspace_name") {
        if !workspace_name.trim().is_empty() {
            return workspace_dir(workspace_name.trim());
        }
    }
    PathBuf::from(".")
}

pub fn resolve_safe_relative_path(base: &PathBuf, raw_path: &str) -> Result<PathBuf, McpError> {
    let candidate = PathBuf::from(raw_path);
    if candidate.is_absolute() {
        return Err(McpError::Message("不允许绝对路径，请使用相对路径".to_string()));
    }

    for comp in candidate.components() {
        if matches!(comp, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(McpError::Message("路径不安全：不允许 .. 或根路径跳转".to_string()));
        }
    }

    Ok(base.join(candidate))
}
