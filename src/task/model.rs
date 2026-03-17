use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::workspace::model::AgentId;

pub type TaskId = uuid::Uuid;

/// `tasks` 表对应的数据结构（与数据库字段一一对应）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskTableModel {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
    pub priority: String,
    pub status: TaskStatus,
    pub name: String,
    pub description: String,
    pub workspace_name: Option<String>,
    pub assigned_agent_id: Option<AgentId>,
    pub created_at: DateTime<Utc>,
}

/// 对外返回的任务信息（包含可读的 agent 名称）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
    pub priority: String,
    pub status: TaskStatus,
    pub name: String,
    pub description: String,
    pub workspace_name: Option<String>,
    /// 把原来的 assigned_agent_id 改为 agent 的可读名称
    pub assigned_agent_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl TaskInfo {
    pub fn from_row(row: sqlx::postgres::PgRow) -> Self {
        Self {
            id: row.get("id"),
            depends_on: row.get("depends_on"),
            priority: row.get("priority"),
            status: match row.get::<String, _>("status").as_str() {
                "published" => TaskStatus::Published,
                "accepted" => TaskStatus::Accepted,
                "executing" => TaskStatus::Executing,
                "submitted" => TaskStatus::Submitted,
                "under_review" => TaskStatus::Reviewing,
                "completed_success" => TaskStatus::CompletedSuccess,
                "completed_failure" => TaskStatus::CompletedFailure,
                "cancelled" => TaskStatus::Cancelled,
                _ => TaskStatus::Published,
            },
            name: row.get("name"),
            description: row.get("description"),
            workspace_name: row.get("workspace_name"),
            assigned_agent_name: row.get::<Option<String>, _>("assigned_agent_name"),
            created_at: row.get("created_at"),
        }
    }
}

/// 前端期望的任务信息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfoResponse {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    pub priority: String,
    pub status: String,
    pub status_label: String,
    pub status_group: String,
    pub due_date: Option<String>,
    pub assigned_agent_id: Option<String>,
    pub assigned_agent_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskInfoResponse {
    pub fn from_row(row: sqlx::postgres::PgRow) -> Self {
        let status_str: String = row.get("status");
        let (status_label, status_group) = match status_str.as_str() {
            "published" => ("等待中".to_string(), "pending".to_string()),
            "accepted" => ("已接取".to_string(), "pending".to_string()),
            "executing" => ("执行中".to_string(), "running".to_string()),
            "submitted" => ("已提交".to_string(), "running".to_string()),
            "under_review" => ("审阅中".to_string(), "running".to_string()),
            "completed_success" => ("已完成".to_string(), "completed".to_string()),
            "completed_failure" => ("失败".to_string(), "failed".to_string()),
            "cancelled" => ("已取消".to_string(), "failed".to_string()),
            _ => ("等待中".to_string(), "pending".to_string()),
        };

        let created_at: DateTime<Utc> = row.get("created_at");

        Self {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            priority: row.get("priority"),
            status: status_str,
            status_label,
            status_group,
            due_date: None,
            assigned_agent_id: None,
            assigned_agent_name: row.get::<Option<String>, _>("assigned_agent_name"),
            created_at,
            updated_at: created_at,
        }
    }
}

impl TaskTableModel {
    pub fn from_row(row: sqlx::postgres::PgRow) -> Self {
        Self {
            id: row.get("id"),
            depends_on: row.get("depends_on"),
            priority: row.get("priority"),
            status: match row.get::<String, _>("status").as_str() {
                "published" => TaskStatus::Published,
                "accepted" => TaskStatus::Accepted,
                "executing" => TaskStatus::Executing,
                "submitted" => TaskStatus::Submitted,
                "under_review" => TaskStatus::Reviewing,
                "completed_success" => TaskStatus::CompletedSuccess,
                "completed_failure" => TaskStatus::CompletedFailure,
                "cancelled" => TaskStatus::Cancelled,
                _ => TaskStatus::Published, // 默认回退到 Published
            },
            name: row.get("name"),
            description: row.get("description"),
            workspace_name: row.get("workspace_name"),
            assigned_agent_id: row.get("assigned_agent_id"),
            created_at: row.get("created_at"),
        }
    }

    pub fn from_task_item(task_id: TaskId, task: TaskItem) -> Self {
        let now = Utc::now();
        Self {
            id: task_id,
            depends_on: task
                .depends_on
                .iter()
                .filter_map(|id| id.parse().ok())
                .collect(),
            priority: task.priority.as_str().to_string(),
            status: TaskStatus::Published,
            name: task.name.clone(),
            description: task.description.clone(),
            workspace_name: None, // 需要在调用时设置正确的工作区名称
            assigned_agent_id: None,
            created_at: now,
        }
    }
}

/// 消息分类响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageClassificationResponse {
    /// 是否为任务
    #[serde(rename = "is_task")]
    pub is_task: bool,

    /// 任务列表（可能为null）
    #[serde(rename = "tasks")]
    pub tasks: Option<Vec<TaskItem>>,
}

/// 任务状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 任务已注册并质押奖励，等待接取
    Published,
    /// 已被一个或多个 Agent 认领，处于准备或执行阶段
    Accepted,
    /// Agent 正在实际执行任务
    Executing,
    /// Agent 已提交结果，等待审阅
    Submitted,
    /// 审阅者正在评估结果
    Reviewing,
    /// 审阅通过，奖励已分配
    CompletedSuccess,
    /// 审阅未通过或超时，任务关闭，可能触发惩罚
    CompletedFailure,
    /// 任务发布者在接取前取消任务
    Cancelled,
}

impl TaskStatus {
    /// 返回与数据库中 `status` 字段对应的字符串值
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Published => "published",
            TaskStatus::Accepted => "accepted",
            TaskStatus::Executing => "executing",
            TaskStatus::Submitted => "submitted",
            TaskStatus::Reviewing => "under_review",
            TaskStatus::CompletedSuccess => "completed_success",
            TaskStatus::CompletedFailure => "completed_failure",
            TaskStatus::Cancelled => "cancelled",
        }
    }
    // 从数据库字符串解析为 TaskStatus 枚举
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "accepted" => TaskStatus::Accepted,
            "executing" => TaskStatus::Executing,
            "submitted" => TaskStatus::Submitted,
            "under_review" => TaskStatus::Reviewing,
            "completed_success" => TaskStatus::CompletedSuccess,
            "completed_failure" => TaskStatus::CompletedFailure,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Published,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub priority: TaskPriority,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskPriority {
    /// 将字符串转换为 TaskPriority 枚举
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(TaskPriority::Low),
            "medium" => Some(TaskPriority::Medium),
            "high" => Some(TaskPriority::High),
            "critical" => Some(TaskPriority::Critical),
            _ => None,
        }
    }

    /// 将 TaskPriority 枚举转换为字符串
    pub fn as_str(&self) -> &str {
        match self {
            TaskPriority::Low => "low",
            TaskPriority::Medium => "medium",
            TaskPriority::High => "high",
            TaskPriority::Critical => "critical",
        }
    }
}

impl MessageClassificationResponse {
    /// 创建新的消息分类响应
    pub fn new(is_task: bool, tasks: Option<Vec<TaskItem>>) -> Self {
        Self { is_task, tasks }
    }

    /// 获取任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.as_ref().map(|t| t.len()).unwrap_or(0)
    }

    /// 是否有任务
    pub fn has_tasks(&self) -> bool {
        self.tasks.is_some() && !self.tasks.as_ref().unwrap().is_empty()
    }

    /// 转换为JSON字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从JSON字符串解析
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

impl Default for MessageClassificationResponse {
    fn default() -> Self {
        Self {
            is_task: false,
            tasks: None,
        }
    }
}
