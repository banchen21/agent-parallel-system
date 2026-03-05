use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub workspace_id: Uuid,
    pub definition: serde_json::Value,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkflowExecution {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub triggered_by: Uuid,
    pub input: serde_json::Value,
    pub options: serde_json::Value,
    pub status: String,
    pub task_id: Option<Uuid>,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateWorkflowRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub description: Option<String>,
    pub workspace_id: Uuid,
    pub definition: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteWorkflowRequest {
    pub input: Option<serde_json::Value>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub workspace_id: Uuid,
    pub definition: serde_json::Value,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResponse {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub triggered_by: Uuid,
    pub input: serde_json::Value,
    pub options: serde_json::Value,
    pub status: String,
    pub task_id: Option<Uuid>,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Workflow {
    pub fn to_response(&self) -> WorkflowResponse {
        WorkflowResponse {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            workspace_id: self.workspace_id,
            definition: self.definition.clone(),
            is_active: self.is_active,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl WorkflowExecution {
    pub fn to_response(&self) -> WorkflowExecutionResponse {
        WorkflowExecutionResponse {
            id: self.id,
            workflow_id: self.workflow_id,
            triggered_by: self.triggered_by,
            input: self.input.clone(),
            options: self.options.clone(),
            status: self.status.clone(),
            task_id: self.task_id,
            result: self.result.clone(),
            error_message: self.error_message.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl CreateWorkflowRequest {
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        if self.name.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "工作流名称不能为空".to_string(),
            ));
        }

        if self.name.chars().count() > 100 {
            return Err(crate::core::errors::AppError::ValidationError(
                "工作流名称长度不能超过100字符".to_string(),
            ));
        }

        if !self.definition.is_object() {
            return Err(crate::core::errors::AppError::ValidationError(
                "工作流定义必须是对象".to_string(),
            ));
        }

        Ok(())
    }
}
