use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// 通道类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "varchar")]
pub enum ChannelType {
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "qq")]
    QQ,
    #[serde(rename = "web")]
    Web,
}

impl ChannelType {
    pub fn as_str(&self) -> &str {
        match self {
            ChannelType::Telegram => "telegram",
            ChannelType::Discord => "discord",
            ChannelType::QQ => "qq",
            ChannelType::Web => "web",
        }
    }
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelConfig {
    pub id: Uuid,
    pub channel_type: String,
    pub name: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 通道用户映射
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelUser {
    pub id: Uuid,
    pub channel_config_id: Uuid,
    pub user_id: Option<Uuid>,
    pub channel_user_id: String,
    pub channel_username: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建通道配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelConfigRequest {
    pub channel_type: String,
    pub name: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
}

/// 更新通道配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelConfigRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub config: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

/// 通道配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfigResponse {
    pub id: Uuid,
    pub channel_type: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChannelConfig {
    pub fn to_response(&self) -> ChannelConfigResponse {
        ChannelConfigResponse {
            id: self.id,
            channel_type: self.channel_type.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
