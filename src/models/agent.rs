use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;
use validator::Validate;

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

/// 智能体状态
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum AgentStatus {
    Online,
    Offline,
    Busy,
    Idle,
    Error,
}

impl From<String> for AgentStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "online" => AgentStatus::Online,
            "busy" => AgentStatus::Busy,
            "idle" => AgentStatus::Idle,
            "error" => AgentStatus::Error,
            _ => AgentStatus::Offline,
        }
    }
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            AgentStatus::Online => "online",
            AgentStatus::Offline => "offline",
            AgentStatus::Busy => "busy",
            AgentStatus::Idle => "idle",
            AgentStatus::Error => "error",
        };
        write!(f, "{}", value)
    }
}

/// 智能体能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: serde_json::Value,
}

/// 智能体端点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoints {
    pub task_execution: String,
    pub health_check: String,
    pub status_update: Option<String>,
}

/// 智能体限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLimits {
    pub max_concurrent_tasks: i32,
    pub max_execution_time: i32,
    pub max_memory_usage: Option<i64>,
    pub rate_limit_per_minute: Option<i32>,
}

/// 智能体模型
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: AgentStatus,
    pub capabilities: serde_json::Value,
    pub endpoints: serde_json::Value,
    pub limits: serde_json::Value,
    pub current_load: i32,
    pub max_concurrent_tasks: i32,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 智能体注册请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RegisterAgentRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    
    pub description: Option<String>,
    pub capabilities: Vec<Capability>,
    pub endpoints: AgentEndpoints,
    pub limits: AgentLimits,
    pub metadata: Option<serde_json::Value>,
}

/// 智能体心跳请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatRequest {
    pub current_load: i32,
    pub resource_usage: ResourceUsage,
    pub active_tasks: Vec<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

/// 资源使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
    pub network: Option<f64>,
}

/// 智能体响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: AgentStatus,
    pub capabilities: Vec<Capability>,
    pub current_load: i32,
    pub max_concurrent_tasks: i32,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 智能体健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealthStatus {
    pub status: String,
    pub assigned_tasks: Vec<Uuid>,
    pub system_info: SystemInfo,
}

/// 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub maintenance_window: Option<String>,
    pub rate_limits: RateLimits,
    pub supported_models: Vec<String>,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    pub max_requests_per_minute: i32,
    pub max_tokens_per_minute: Option<i32>,
    pub max_concurrent_requests: Option<i32>,
}

/// 任务分配请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignmentRequest {
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub priority: TaskPriority,
    pub timeout: Option<i32>,
}

/// 任务分配响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignmentResponse {
    pub success: bool,
    pub assignment_id: Uuid,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

impl Agent {
    /// 转换为响应格式
    pub fn to_response(&self) -> AgentResponse {
        let capabilities: Vec<Capability> = serde_json::from_value(self.capabilities.clone())
            .unwrap_or_default();
        
        AgentResponse {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            status: self.status.clone(),
            capabilities,
            current_load: self.current_load,
            max_concurrent_tasks: self.max_concurrent_tasks,
            last_heartbeat: self.last_heartbeat,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
    
    /// 检查智能体是否可用
    pub fn is_available(&self) -> bool {
        self.status == AgentStatus::Online && self.current_load < self.max_concurrent_tasks
    }
    
    /// 检查智能体是否具有特定能力
    pub fn has_capability(&self, capability: &str) -> bool {
        if let Ok(capabilities) = serde_json::from_value::<Vec<Capability>>(self.capabilities.clone()) {
            capabilities.iter().any(|c| c.name == capability)
        } else {
            false
        }
    }
    
    /// 获取智能体能力列表
    pub fn get_capabilities(&self) -> Vec<String> {
        if let Ok(capabilities) = serde_json::from_value::<Vec<Capability>>(self.capabilities.clone()) {
            capabilities.into_iter().map(|c| c.name).collect()
        } else {
            Vec::new()
        }
    }
    
    /// 检查智能体是否健康（最近有心跳）
    pub fn is_healthy(&self) -> bool {
        if let Some(last_heartbeat) = self.last_heartbeat {
            let now = Utc::now();
            let duration = now - last_heartbeat;
            duration.num_seconds() < 300 // 5分钟内有心跳
        } else {
            false
        }
    }
}

impl RegisterAgentRequest {
    /// 验证注册智能体请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        if self.name.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "智能体名称不能为空".to_string(),
            ));
        }
        
        if self.capabilities.is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "智能体必须至少有一个能力".to_string(),
            ));
        }
        
        // 验证端点URL
        if self.endpoints.task_execution.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "任务执行端点不能为空".to_string(),
            ));
        }
        
        if self.endpoints.health_check.trim().is_empty() {
            return Err(crate::core::errors::AppError::ValidationError(
                "健康检查端点不能为空".to_string(),
            ));
        }
        
        // 验证限制
        if self.limits.max_concurrent_tasks <= 0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "最大并发任务数必须大于0".to_string(),
            ));
        }
        
        if self.limits.max_execution_time <= 0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "最大执行时间必须大于0".to_string(),
            ));
        }
        
        Ok(())
    }
}

impl AgentHeartbeatRequest {
    /// 验证智能体心跳请求
    pub fn validate(&self) -> Result<(), crate::core::errors::AppError> {
        if self.current_load < 0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "当前负载不能为负数".to_string(),
            ));
        }
        
        // 验证资源使用率
        if self.resource_usage.cpu < 0.0 || self.resource_usage.cpu > 100.0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "CPU使用率必须在0-100之间".to_string(),
            ));
        }
        
        if self.resource_usage.memory < 0.0 || self.resource_usage.memory > 100.0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "内存使用率必须在0-100之间".to_string(),
            ));
        }
        
        if self.resource_usage.disk < 0.0 || self.resource_usage.disk > 100.0 {
            return Err(crate::core::errors::AppError::ValidationError(
                "磁盘使用率必须在0-100之间".to_string(),
            ));
        }
        
        Ok(())
    }
}
