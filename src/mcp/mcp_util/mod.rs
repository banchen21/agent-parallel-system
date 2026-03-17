mod common;
mod filesystem;
mod system;
mod terminal;
mod weather;

use serde_json::Value;

use crate::mcp::model::McpError;

pub async fn execute_builtin_tool(tool_id: &str, arguments: &Value) -> Result<Value, McpError> {
    match tool_id {
        "get_weather" => weather::execute_get_weather(arguments).await,
        "system_info" => system::execute_system_info(arguments).await,
        "fs_list_dir" => filesystem::execute_fs_list_dir(arguments).await,
        "fs_read_file" => filesystem::execute_fs_read_file(arguments).await,
        "fs_write_file" => filesystem::execute_fs_write_file(arguments).await,
        "terminal_run" | "shell_exec" => terminal::execute_terminal_run(arguments).await,
        _ => Err(McpError::Message(format!(
            "工具 {} 未配置 execution，且无内置执行后端",
            tool_id
        ))),
    }
}