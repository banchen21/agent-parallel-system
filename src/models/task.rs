use chrono::{DateTime, Utc};
use std::fmt;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::core::security::InputValidator;

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
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
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
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
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub parent_task_id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub assigned_agent_id: Option<Uuid>,
    pub created_by: Uuid,
    pub requirements: serde_json::Value,
    pub context: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub progress: i32,
    pub current_step: Option<String>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time: Option<i32>,
    pub retry_count: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 任务创建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTaskRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub workspace_id: Uuid,
    pub requirements: serde_json::Value,
    pub context: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

/// 任务更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub progress: Option<i32>,
    pub current_step: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 任务状态更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: TaskStatus,
    pub progress: Option<i32>,
    pub current_step: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 任务响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub parent_task_id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub assigned_agent_id: Option<Uuid>,
    pub created_by: Uuid,
    pub requirements: serde_json::Value,
    pub context: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub progress: i32,
    pub current_step: Option<String>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time: Option<i32>,
    pub retry_count: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub subtasks: Vec<TaskResponse>,
}

/// 任务依赖模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskDependency {
    pub id: Uuid,
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
    pub dependency_type: DependencyType,
    pub created_at: DateTime<Utc>,
}

/// 依赖类型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum DependencyType {
    Blocking,
    NonBlocking,
}

/// 任务分解策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskDecompositionStrategy {
    Hierarchical,
    Sequential,
    Parallel,
}

/// 任务分解请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecompositionRequest {
    pub strategy: TaskDecompositionStrategy,
    pub max_depth: Option<i32>,
    pub constraints: Option<serde_json::Value>,
}

/// 任务分解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecompositionResult {
    pub parent_task_id: Uuid,
    pub subtasks: Vec<SubtaskDefinition>,
}

/// 子任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskDefinition {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub dependencies: Vec<Uuid>,
    pub estimated_duration: i32,
    pub required_capabilities: Vec<String>,
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
            parent_task_id: self.parent_task_id,
            workspace_id: self.workspace_id,
            assigned_agent_id: self.assigned_agent_id,
            created_by: self.created_by,
            requirements: self.requirements.clone(),
            context: self.context.clone(),
            result: self.result.clone(),
            progress: self.progress,
            current_step: self.current_step.clone(),
            estimated_completion: self.estimated_completion,
            started_at: self.started_at,
            completed_at: self.completed_at,
            execution_time: self.execution_time,
            retry_count: self.retry_count,
            metadata: self.metadata.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            subtasks,
        }
    }
    
    /// 验证任务数据
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        InputValidator::validate_task_title(&self.title)?;
        Ok(())
    }
    
    /// 检查任务是否可以开始
    pub fn can_start(&self) -> bool {
        self.status == TaskStatus::Pending
    }
    
    /// 检查任务是否可以完成
    pub fn can_complete(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress | TaskStatus::Pending)
    }
    
    /// 检查任务是否可以取消
    pub fn can_cancel(&self) -> bool {
        matches!(self.status, TaskStatus::Pending | TaskStatus::InProgress)
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
