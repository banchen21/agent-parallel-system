//! 工作区目录约定：`.workspaces/<workspace_name>/agents/<agent_id>/`
//!
//! 工作区根目录由环境变量 `WORKSPACE_ROOT` 指定，默认为 `.workspaces`（相对当前工作目录）。

use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 工作区根目录（如 `.workspaces`），来自环境变量 `WORKSPACE_ROOT`
pub fn workspace_root() -> PathBuf {
    std::env::var("WORKSPACE_ROOT")
        .map(PathBuf::from)
    .unwrap_or_else(|_| PathBuf::from(".workspaces"))
}

/// 某工作区的根路径：`<root>/<workspace_name>/`
pub fn workspace_dir(workspace_name: &str) -> PathBuf {
    workspace_root().join(workspace_name)
}

/// 某工作区下的 agents 目录：`<root>/<workspace_name>/agents/`
pub fn workspace_agents_dir(workspace_name: &str) -> PathBuf {
    workspace_dir(workspace_name).join("agents")
}

/// 某 Agent 在工作区中的目录：`<root>/<workspace_name>/agents/<agent_id>/`
pub fn agent_dir(workspace_name: &str, agent_id: Uuid) -> PathBuf {
    workspace_agents_dir(workspace_name).join(agent_id.to_string())
}

/// Agent 记忆存储目录：`<root>/<workspace_name>/agents/<agent_id>/memory/`
pub fn agent_memory_dir(workspace_name: &str, agent_id: Uuid) -> PathBuf {
    agent_dir(workspace_name, agent_id).join("memory")
}

/// 确保目录存在，若不存在则创建（含父级）
pub fn ensure_dir(p: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(p)
}
