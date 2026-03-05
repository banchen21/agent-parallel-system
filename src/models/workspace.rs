use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;
use validator::Validate;

/// 工作空间权限级别
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PermissionLevel {
    Read,
    Write,
    Admin,
}

impl From<String> for PermissionLevel {
    fn from(value: String) -> Self {
        match value.as_str() {
            "admin" => PermissionLevel::Admin,
            "write" => PermissionLevel::Write,
            _ => PermissionLevel::Read,
        }
    }
}

impl fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            PermissionLevel::Read => "read",
            PermissionLevel::Write => "write",
            PermissionLevel::Admin => "admin",
        };
        write!(f, "{}", value)
    }
}

/// 工作空间权限
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkspacePermission {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub permission_level: PermissionLevel,
    pub granted_by: Uuid,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 工作空间模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_public: bool,
    pub context: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 工作空间创建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateWorkspaceRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    
    pub description: Option<String>,
    pub is_public: Option<bool>,
    pub context: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 工作空间更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
    pub context: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 工作空间响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_public: bool,
    pub context: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub permissions: Vec<WorkspacePermissionResponse>,
    pub document_count: i64,
    pub active_task_count: i64,
}

/// 工作空间权限响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePermissionResponse {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub permission_level: PermissionLevel,
    pub granted_by: Uuid,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 权限授予请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantPermissionRequest {
    pub user_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub permission_level: PermissionLevel,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 工作空间上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub workspace_id: Uuid,
    pub context: WorkspaceContextData,
    pub documents: Vec<DocumentResponse>,
    pub tools: Vec<ToolResponse>,
}

/// 工作空间上下文数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContextData {
    pub project_info: ProjectInfo,
    pub shared_knowledge: Vec<KnowledgeItem>,
    pub recent_activities: Vec<ActivityItem>,
}

/// 项目信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub description: String,
    pub timeline: ProjectTimeline,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
}

/// 项目时间线
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTimeline {
    pub start_date: String,
    pub end_date: String,
    pub milestones: Vec<Milestone>,
}

/// 里程碑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub name: String,
    pub date: String,
    pub description: String,
    pub completed: bool,
}

/// 知识项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: Uuid,
    pub type_: String,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
}

/// 活动项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub timestamp: DateTime<Utc>,
    pub agent: String,
    pub action: String,
    pub details: String,
    pub task_id: Option<Uuid>,
}

/// 文档模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Document {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub file_name: Option<String>,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    pub storage_url: Option<String>,
    pub content_type: Option<String>,
    pub content_hash: Option<String>,
    pub metadata: serde_json::Value,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 文档响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub name: String,
    pub file_name: Option<String>,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    pub storage_url: Option<String>,
    pub content_type: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// 工具模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tool {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub endpoint_url: Option<String>,
    pub authentication_type: String,
    pub authentication_config: serde_json::Value,
    pub parameters_schema: serde_json::Value,
    pub capabilities: serde_json::Value,
    pub is_active: bool,
    pub rate_limit_per_minute: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 工具响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub endpoint_url: Option<String>,
    pub parameters: serde_json::Value,
    pub capabilities: Vec<String>,
}

impl Workspace {
    /// 转换为响应格式
    pub fn to_response(
        &self,
        permissions: Vec<WorkspacePermissionResponse>,
        document_count: i64,
        active_task_count: i64,
    ) -> WorkspaceResponse {
        WorkspaceResponse {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            owner_id: self.owner_id,
            is_public: self.is_public,
            context: self.context.clone(),
            metadata: self.metadata.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            permissions,
            document_count,
            active_task_count,
        }
    }
    
    /// 检查用户是否有权限
    pub fn has_permission(
        &self,
        user_id: Uuid,
        required_level: PermissionLevel,
        permissions: &[WorkspacePermission],
    ) -> bool {
        // 所有者拥有所有权限
        if self.owner_id == user_id {
            return true;
        }
        
        // 检查用户权限
        for permission in permissions {
            if permission.user_id == Some(user_id) {
                return match (permission.permission_level.clone(), required_level) {
                    (PermissionLevel::Admin, _) => true,
                    (PermissionLevel::Write, PermissionLevel::Write) => true,
                    (PermissionLevel::Write, PermissionLevel::Read) => true,
                    (PermissionLevel::Read, PermissionLevel::Read) => true,
                    _ => false,
                };
            }
        }
        
        // 公共工作空间的读取权限
        if self.is_public && required_level == PermissionLevel::Read {
            return true;
        }
        
        false
    }
}

impl WorkspacePermission {
    /// 转换为响应格式
    pub fn to_response(&self) -> WorkspacePermissionResponse {
        WorkspacePermissionResponse {
            id: self.id,
            user_id: self.user_id,
            agent_id: self.agent_id,
            permission_level: self.permission_level.clone(),
            granted_by: self.granted_by,
            granted_at: self.granted_at,
            expires_at: self.expires_at,
        }
    }
    
    /// 检查权限是否过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }
}

impl Document {
    /// 转换为响应格式
    pub fn to_response(&self) -> DocumentResponse {
        DocumentResponse {
            id: self.id,
            name: self.name.clone(),
            file_name: self.file_name.clone(),
            file_type: self.file_type.clone(),
            file_size: self.file_size,
            storage_url: self.storage_url.clone(),
            content_type: self.content_type.clone(),
            uploaded_at: self.created_at,
            metadata: self.metadata.clone(),
        }
    }
}

impl Tool {
    /// 转换为响应格式
    pub fn to_response(&self) -> ToolResponse {
        let capabilities: Vec<String> = serde_json::from_value(self.capabilities.clone())
            .unwrap_or_default();
        
        ToolResponse {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            category: self.category.clone(),
            endpoint_url: self.endpoint_url.clone(),
            parameters: self.parameters_schema.clone(),
            capabilities,
        }
    }
}
