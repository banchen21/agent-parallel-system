use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub tool_id: String,
    pub description: String,
    pub parameters: McpParameters,
    pub options: McpOptions,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct McpParameters {
    pub r#type: String,
    pub properties: serde_json::Map<String, Value>,
    #[serde(default)]
    pub required: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOptions {
    pub timeout_ms: u64,
    pub max_retries: u32,
}
