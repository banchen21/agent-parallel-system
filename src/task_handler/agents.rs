//! 任务执行 Agent 定义
//!
//! 用于 DAG 编排器中注册、识别可接取并执行任务的 Worker Agent。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// 任务执行 Agent 信息
///
/// 与 `DagOrchestrator` 中 `agents: HashMap<AgentId, String>` 对应：
/// - key 为 `id`，value 可为 `name` 或 `display_name()`。
/// 创建 Agent 时必须绑定到已存在的工作空间（通过 `workspace_name` 指定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    #[serde(default)]
    pub kind: AgentKind,
    /// 所属工作空间名称（必须为已存在的工作区）
    pub workspace_name: String,
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
        }
    }

    /// 生成新 ID 并创建 Agent，需指定已存在的工作空间名称
    pub fn create(
        name: impl Into<String>,
        kind: AgentKind,
        workspace_name: impl Into<String>,
    ) -> Self {
        Self::new(Uuid::new_v4(), name, kind, workspace_name)
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
