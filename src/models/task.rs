use chrono::{DateTime, Utc};
use std::fmt;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::core::security::InputValidator;

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// 任务优先级
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

/// 任务依赖类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Blocking,
    NonBlocking,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", value)
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            TaskPriority::Low => "low",
            TaskPriority::Medium => "medium",
            TaskPriority::High => "high",
            TaskPriority::Urgent => "urgent",
        };
        write!(f, "{}", value)
    }
}

impl From<String> for TaskStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "pending" => TaskStatus::Pending,
            "in_progress" => TaskStatus::InProgress,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        }
    }
}

impl From<String> for TaskPriority {
    fn from(value: String) -> Self {
        match value.as_str() {
            "low" => TaskPriority::Low,
            "medium" => TaskPriority::Medium,
            "high" => TaskPriority::High,
            "urgent" => TaskPriority::Urgent,
            _ => TaskPriority::Medium,
        }
    }
}

impl From<String> for DependencyType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "nonblocking" | "non_blocking" => DependencyType::NonBlocking,
            _ => DependencyType::Blocking,
        }
    }
}

/// 任务模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub parent_task_id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub assigned_agent_id: Option<Uuid>,
    pub created_by: Uuid,
    pub progress: i32,
    pub requirements: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: Option<i32>,
    pub metadata: serde_json::Value,
    pub execution_context: serde_json::Value,
    pub tags: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 任务创建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTaskRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub context: Option<serde_json::Value>,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub workspace_id: Uuid,
    pub requirements: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

/// 任务更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 任务状态更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 任务响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub workspace_id: Uuid,
    pub assigned_agent_id: Option<Uuid>,
    pub created_by: Uuid,
    pub requirements: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub subtasks: Vec<TaskResponse>,
}

/// 任务分解请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecompositionRequest {
    pub task_id: Uuid,
    pub subtasks: Vec<SubtaskDefinition>,
}

/// 子任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskDefinition {
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub requirements: serde_json::Value,
}

/// 任务分解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecompositionResult {
    pub parent_task_id: Uuid,
    pub subtasks: Vec<Task>,
}

impl Task {
    /// 转换为响应格式
    pub fn to_response(&self, subtasks: Vec<TaskResponse>) -> TaskResponse {
        TaskResponse {
            id: self.id,
            title: self.title.clone(),
            description: self.description.clone(),
            status: self.status.clone(),
            priority: self.priority.clone(),
            workspace_id: self.workspace_id,
            assigned_agent_id: self.assigned_agent_id,
            created_by: self.created_by,
            requirements: self.requirements.clone(),
            result: self.result.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
            retry_count: self.retry_count.unwrap_or(0),
            metadata: self.metadata.clone(),
            created_at: self.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: self.updated_at.unwrap_or_else(|| chrono::Utc::now()),
            subtasks,
        }
    }
    
    /// 检查任务是否可以开始
    pub fn can_start(&self) -> bool {
        self.status == "pending"
    }
    
    /// 检查任务是否可以完成
    pub fn can_complete(&self) -> bool {
        self.status == "pending" || self.status == "in_progress"
    }
    
    /// 检查任务是否可以取消
    pub fn can_cancel(&self) -> bool {
        self.status == "pending" || self.status == "in_progress"
    }
    
    /// 计算任务执行时间
    pub fn calculate_execution_time(&self) -> Option<i32> {
        if let (Some(started), Some(completed)) = (self.started_at, self.completed_at) {
            Some((completed - started).num_seconds() as i32)
        } else {
            None
        }
    }
}

impl CreateTaskRequest {
    /// 验证创建任务请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        InputValidator::validate_task_title(&self.title)?;
        Ok(())
    }
}

impl UpdateTaskRequest {
    /// 验证更新任务请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        if let Some(title) = &self.title {
            InputValidator::validate_task_title(title)?;
        }
        Ok(())
    }
}
