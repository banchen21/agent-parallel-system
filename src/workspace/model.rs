use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// 错误类型
#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("数据库操作失败: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Actor 通信失败 (可能已宕机或超时): {0}")]
    MailboxError(#[from] actix::MailboxError), // 自动处理 actix 的 send 报错

    #[error("文件系统/IO 操作失败: {0}")]
    IoError(#[from] std::io::Error), // 如果你的工作区涉及本地文件夹的创建删除，必须加这个

    // === 2. 业务逻辑错误 (400 / 404 / 409) ===
    #[error("未找到对应的工作区: {0}")]
    NotFound(String), // 查询或删除时找不到对应数据

    #[error("该工作区已存在: {0}")]
    AlreadyExists(String), // 创建时发生名称冲突

    #[error("操作失败: {0}")]
    Message(String), // 通用的其他业务报错
}

/// 任务执行 Agent 的唯一标识
pub type AgentId = Uuid;

/// Agent 种类（可用于路由或能力匹配）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// 通用工作 Agent
    General,
    /// 代码/开发类任务
    Code,
    /// 研究/检索类任务
    Research,
    /// 自定义类型，由 name 区分
    Custom,
}

impl Default for AgentKind {
    fn default() -> Self {
        AgentKind::General
    }
}

impl AgentKind {
    /// 用于数据库存储的字符串
    pub fn as_db_str(&self) -> &'static str {
        match self {
            AgentKind::General => "general",
            AgentKind::Code => "code",
            AgentKind::Research => "research",
            AgentKind::Custom => "custom",
        }
    }

    /// 从数据库字符串解析
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "code" => AgentKind::Code,
            "research" => AgentKind::Research,
            "custom" => AgentKind::Custom,
            _ => AgentKind::General,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    #[serde(default)]
    pub kind: AgentKind,
    /// 所属工作空间名称（必须为已存在的工作区）
    pub workspace_name: String,
    pub owner_username: String,
    /// 智能体可用的 MCP 工具列表
    #[serde(default)]
    pub mcp_list: Vec<String>,
}

impl AgentInfo {
    /// 创建新的 Agent 信息，需指定已存在的工作空间名称
    pub fn new(
        id: AgentId,
        name: impl Into<String>,
        kind: AgentKind,
        workspace_name: impl Into<String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            workspace_name: workspace_name.into(),
            owner_username: String::new(),
            mcp_list: Vec::new(),
        }
    }

    /// 生成新 ID 并创建 Agent，需指定已存在的工作空间名称
    pub fn create(
        name: impl Into<String>,
        kind: AgentKind,
        workspace_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            workspace_name: workspace_name.into(),
            owner_username: String::new(),
            mcp_list: Vec::new(),
        }
    }

    /// 用于存入 HashMap 的显示名（工作区:名称，便于日志与排查）
    pub fn display_name(&self) -> String {
        format!("{}:{}:{}", self.workspace_name, self.id, self.name)
    }
}

impl From<AgentInfo> for (AgentId, String) {
    fn from(a: AgentInfo) -> Self {
        (a.id, a.name)
    }
}

impl From<&AgentInfo> for (AgentId, String) {
    fn from(a: &AgentInfo) -> Self {
        (a.id, a.name.clone())
    }
}
