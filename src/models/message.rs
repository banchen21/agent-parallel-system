//! 消息模型
//! 
//! 定义系统内部消息的数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// 智能体消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentMessage {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub message_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub read: bool,
}

/// 任务消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskMessage {
    pub id: Uuid,
    pub task_id: Uuid,
    pub message_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// 用户消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserMessage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub message_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub read: bool,
}

/// 系统广播消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBroadcast {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub broadcast_type: String,
    pub priority: String,
    pub target_audience: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 消息发送请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub message_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub target_id: Uuid,
    pub target_type: String, // "agent", "task", "user", "system"
}

/// 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub message_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub read: Option<bool>,
    pub sender_info: Option<serde_json::Value>,
}

/// 消息列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageListResponse {
    pub messages: Vec<MessageResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub has_more: bool,
}

/// 消息标记为已读请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkAsReadRequest {
    pub message_ids: Vec<Uuid>,
    pub message_type: String, // "agent", "user"
}

/// 消息删除请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMessagesRequest {
    pub message_ids: Vec<Uuid>,
    pub message_type: String, // "agent", "user", "task"
}

/// 消息订阅请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub channel: String,
    pub filter: Option<serde_json::Value>,
}

/// 消息订阅响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    pub subscription_id: Uuid,
    pub channel: String,
    pub expires_at: DateTime<Utc>,
}

impl From<AgentMessage> for MessageResponse {
    fn from(msg: AgentMessage) -> Self {
        Self {
            id: msg.id,
            message_type: msg.message_type,
            content: msg.content,
            metadata: msg.metadata,
            created_at: msg.created_at,
            read: Some(msg.read),
            sender_info: Some(serde_json::json!({
                "type": "agent",
                "id": msg.agent_id
            })),
        }
    }
}

impl From<TaskMessage> for MessageResponse {
    fn from(msg: TaskMessage) -> Self {
        Self {
            id: msg.id,
            message_type: msg.message_type,
            content: msg.content,
            metadata: msg.metadata,
            created_at: msg.created_at,
            read: None,
            sender_info: Some(serde_json::json!({
                "type": "task",
                "id": msg.task_id
            })),
        }
    }
}

impl From<UserMessage> for MessageResponse {
    fn from(msg: UserMessage) -> Self {
        Self {
            id: msg.id,
            message_type: msg.message_type,
            content: msg.content,
            metadata: msg.metadata,
            created_at: msg.created_at,
            read: Some(msg.read),
            sender_info: Some(serde_json::json!({
                "type": "user",
                "id": msg.user_id
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_message_creation() {
        let id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let now = Utc::now();
        
        let message = AgentMessage {
            id,
            agent_id,
            message_type: "task_assigned".to_string(),
            content: "任务已分配".to_string(),
            metadata: Some(serde_json::json!({"task_id": Uuid::new_v4()})),
            created_at: now,
            read: false,
        };

        assert_eq!(message.id, id);
        assert_eq!(message.agent_id, agent_id);
        assert_eq!(message.message_type, "task_assigned");
        assert_eq!(message.read, false);
    }

    #[test]
    fn test_message_response_conversion() {
        let id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let now = Utc::now();
        
        let agent_message = AgentMessage {
            id,
            agent_id,
            message_type: "test".to_string(),
            content: "测试消息".to_string(),
            metadata: None,
            created_at: now,
            read: false,
        };

        let response: MessageResponse = agent_message.into();
        
        assert_eq!(response.id, id);
        assert_eq!(response.message_type, "test");
        assert_eq!(response.read, Some(false));
    }
}