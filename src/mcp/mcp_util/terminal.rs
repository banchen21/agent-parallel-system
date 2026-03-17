use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::mcp::mcp_util::common::{arg_string, arg_u64, resolve_safe_relative_path, workspace_root_from_args};
use crate::mcp::model::McpError;

pub async fn execute_terminal_run(arguments: &Value) -> Result<Value, McpError> {
    let command = arg_string(arguments, "command")
        .ok_or_else(|| McpError::Message("terminal_run 缺少 command 参数".to_string()))?;
    let lowered = command.to_lowercase();
    let blocked = ["rm -rf /", "mkfs", "shutdown", "reboot", ":(){", "poweroff"];
    if blocked.iter().any(|k| lowered.contains(k)) {
        return Err(McpError::Message("terminal_run 命令被安全策略拒绝".to_string()));
    }

    let base = workspace_root_from_args(arguments);
    let cwd = arg_string(arguments, "cwd").unwrap_or_else(|| ".".to_string());
    let cwd_path = resolve_safe_relative_path(&base, &cwd)?;
    let timeout_ms = arg_u64(arguments, "timeout_ms")
        .unwrap_or(15_000)
        .clamp(500, 120_000);

    let fut = async {
        let output = Command::new("sh")
            .arg("-lc")
            .arg(&command)
            .current_dir(&cwd_path)
            .output()
            .await
            .map_err(|e| McpError::Message(format!("terminal_run 执行失败: {}", e)))?;

        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let max_len = 32_000;
        if stdout.len() > max_len {
            stdout.truncate(max_len);
        }
        if stderr.len() > max_len {
            stderr.truncate(max_len);
        }

        Ok::<Value, McpError>(json!({
            "exit_code": output.status.code(),
            "success": output.status.success(),
            "cwd": cwd_path,
            "stdout": stdout,
            "stderr": stderr
        }))
    };

    timeout(Duration::from_millis(timeout_ms), fut)
        .await
        .map_err(|_| McpError::Message(format!("terminal_run 超时: {}ms", timeout_ms)))?
}
