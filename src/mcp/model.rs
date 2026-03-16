use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// MCP 配置错误类型
#[derive(Debug, Error)]
pub enum McpError {
    #[error("文件系统/IO 操作失败: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON 序列化/反序列化失败: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("MCP 配置未找到: {0}")]
    NotFound(String),

    #[error("MCP 配置已存在: {0}")]
    AlreadyExists(String),

    #[error("操作失败: {0}")]
    Message(String),

    #[error("Actor 通信失败: {0}")]
    MailboxError(#[from] actix::MailboxError),
}

//mcp结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    // 唯一标识符
    pub name: String,
    pub description: String,
    // 命令 列表，key为命令名称，value为命令参数说明
    pub commands: Vec<String>,
    // 命令与参数
    pub command_args: HashMap<String, Vec<String>>,
    pub created_at: String,
}
