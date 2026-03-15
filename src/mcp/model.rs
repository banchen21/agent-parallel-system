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

/// MCP 配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// MCP 名称
    pub name: String,
    /// 类型（stdio, sse 等）
    #[serde(rename = "type")]
    pub mcp_type: Option<String>,
    /// 执行命令
    pub command: String,
    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,
    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// 始终允许的操作
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alwaysAllow: Vec<String>,
}

impl McpConfig {
    /// 创建新的 MCP 配置
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            mcp_type: Some("stdio".to_string()),
            command: command.into(),
            args,
            env: HashMap::new(),
            alwaysAllow: Vec::new(),
        }
    }

    /// 添加环境变量
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// 添加始终允许的操作
    pub fn with_always_allow(mut self, operation: impl Into<String>) -> Self {
        self.alwaysAllow.push(operation.into());
        self
    }

    /// 验证配置是否有效
    pub fn validate(&self) -> Result<(), McpError> {
        if self.name.is_empty() {
            return Err(McpError::Message("MCP 名称不能为空".to_string()));
        }
        if self.command.is_empty() {
            return Err(McpError::Message("命令不能为空".to_string()));
        }
        Ok(())
    }
}

/// MCP 配置列表（用于 .mcps/mcp.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfigList {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpConfig>,
}

impl McpConfigList {
    pub fn new() -> Self {
        Self {
            mcp_servers: HashMap::new(),
        }
    }

    pub fn add(&mut self, config: McpConfig) -> Result<(), McpError> {
        if self.mcp_servers.contains_key(&config.name) {
            return Err(McpError::AlreadyExists(config.name.clone()));
        }
        self.mcp_servers.insert(config.name.clone(), config);
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<McpConfig, McpError> {
        self.mcp_servers
            .remove(name)
            .ok_or_else(|| McpError::NotFound(name.to_string()))
    }

    pub fn get(&self, name: &str) -> Option<&McpConfig> {
        self.mcp_servers.get(name)
    }

    pub fn list(&self) -> Vec<&McpConfig> {
        self.mcp_servers.values().collect()
    }
}

impl Default for McpConfigList {
    fn default() -> Self {
        Self::new()
    }
}
