use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "varchar")]
pub enum MessageRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
}

impl MessageRole {
    pub fn as_str(&self) -> &str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 聊天会话
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatSession {
    pub id: Uuid,
    pub channel_user_id: Uuid,
    pub title: Option<String>,
    pub model: String,
    pub system_prompt: Option<String>,
    pub temperature: f32,
    pub max_tokens: i32,
    pub context_window: i32,
    pub metadata: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub tokens_used: Option<i32>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// 创建聊天会话请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatSessionRequest {
    pub channel_user_id: Uuid,
    pub title: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub context_window: Option<i32>,
}

/// 发送聊天消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendChatMessageRequest {
    pub session_id: Uuid,
    pub content: String,
}

/// 聊天消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageResponse {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub tokens_used: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// 聊天会话响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionResponse {
    pub id: Uuid,
    pub title: Option<String>,
    pub model: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatMessage {
    pub fn to_response(&self) -> ChatMessageResponse {
        ChatMessageResponse {
            id: self.id,
            session_id: self.session_id,
            role: self.role.clone(),
            content: self.content.clone(),
            tokens_used: self.tokens_used,
            created_at: self.created_at,
        }
    }
}

impl ChatSession {
    pub fn to_response(&self) -> ChatSessionResponse {
        ChatSessionResponse {
            id: self.id,
            title: self.title.clone(),
            model: self.model.clone(),
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LLMConfig {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub model_name: String,
    pub temperature: f32,
    pub max_tokens: i32,
    pub is_default: bool,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建 LLM 配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLLMConfigRequest {
    pub name: String,
    pub provider: String,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub model_name: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
}
