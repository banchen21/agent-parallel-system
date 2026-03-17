use serde_json::{json, Value};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use crate::mcp::mcp_util::common::{arg_bool, arg_string, arg_u64, resolve_safe_relative_path, workspace_root_from_args};
use crate::mcp::model::McpError;

pub async fn execute_fs_list_dir(arguments: &Value) -> Result<Value, McpError> {
    let base = workspace_root_from_args(arguments);
    let dir = arg_string(arguments, "path").unwrap_or_else(|| ".".to_string());
    let target = resolve_safe_relative_path(&base, &dir)?;

    let mut entries = Vec::new();
    for entry in fs::read_dir(&target)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        entries.push(json!({
            "name": entry.file_name().to_string_lossy().to_string(),
            "is_dir": meta.is_dir(),
            "size": meta.len()
        }));
    }

    Ok(json!({
        "base": base,
        "path": dir,
        "resolved": target,
        "entries": entries
    }))
}

pub async fn execute_fs_read_file(arguments: &Value) -> Result<Value, McpError> {
    let base = workspace_root_from_args(arguments);
    let path = arg_string(arguments, "path")
        .ok_or_else(|| McpError::Message("fs_read_file 缺少 path 参数".to_string()))?;
    let max_bytes = arg_u64(arguments, "max_bytes")
        .unwrap_or(16_384)
        .clamp(256, 1_048_576) as usize;
    let target = resolve_safe_relative_path(&base, &path)?;

    let content = fs::read_to_string(&target)?;
    let was_truncated = content.len() > max_bytes;
    let body = if was_truncated {
        content.chars().take(max_bytes).collect::<String>()
    } else {
        content
    };

    Ok(json!({
        "path": path,
        "resolved": target,
        "was_truncated": was_truncated,
        "content": body
    }))
}

pub async fn execute_fs_write_file(arguments: &Value) -> Result<Value, McpError> {
    let base = workspace_root_from_args(arguments);
    let path = arg_string(arguments, "path")
        .ok_or_else(|| McpError::Message("fs_write_file 缺少 path 参数".to_string()))?;
    let content = arg_string(arguments, "content")
        .ok_or_else(|| McpError::Message("fs_write_file 缺少 content 参数".to_string()))?;
    let append = arg_bool(arguments, "append", false);
    let target = resolve_safe_relative_path(&base, &path)?;

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if append {
        let mut file = OpenOptions::new().create(true).append(true).open(&target)?;
        file.write_all(content.as_bytes())?;
    } else {
        fs::write(&target, content.as_bytes())?;
    }

    Ok(json!({
        "path": path,
        "resolved": target,
        "append": append,
        "written_bytes": content.len()
    }))
}
