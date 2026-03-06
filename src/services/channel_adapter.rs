use async_trait::async_trait;
use serde_json::Value;

use crate::core::errors::AppError;

/// 通道消息
#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub channel_user_id: String,
    pub channel_username: Option<String>,
    pub content: String,
    pub message_type: String, // text, image, file, command
    pub metadata: Value,
}

/// 通道消息响应
#[derive(Debug, Clone)]
pub struct ChannelMessageResponse {
    pub channel_message_id: Option<String>,
    pub content: String,
    pub status: String,
}

/// 通道适配器特征
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// 获取通道类型
    fn channel_type(&self) -> &str;

    /// 初始化通道
    async fn initialize(&self) -> Result<(), AppError>;

    /// 接收消息（从通道拉取）
    async fn receive_messages(&self) -> Result<Vec<ChannelMessage>, AppError>;

    /// 发送消息到通道
    async fn send_message(
        &self,
        channel_user_id: &str,
        content: &str,
    ) -> Result<ChannelMessageResponse, AppError>;

    /// 处理命令
    async fn handle_command(
        &self,
        channel_user_id: &str,
        command: &str,
        args: Vec<String>,
    ) -> Result<String, AppError>;

    /// 获取用户信息
    async fn get_user_info(&self, channel_user_id: &str) -> Result<Value, AppError>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool, AppError>;
}

/// 通道适配器工厂
pub struct ChannelAdapterFactory;

impl ChannelAdapterFactory {
    pub fn create_adapter(
        channel_type: &str,
        config: Value,
    ) -> Result<Box<dyn ChannelAdapter>, AppError> {
        match channel_type {
            "telegram" => {
                let token = config
                    .get("token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AppError::ValidationError("Missing Telegram token".to_string()))?;
                Ok(Box::new(TelegramAdapter::new(token.to_string())))
            }
            "web" => Ok(Box::new(WebAdapter::new())),
            _ => Err(AppError::ValidationError(format!(
                "Unsupported channel type: {}",
                channel_type
            ))),
        }
    }
}

/// Telegram 适配器
pub struct TelegramAdapter {
    token: String,
}

impl TelegramAdapter {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn channel_type(&self) -> &str {
        "telegram"
    }

    async fn initialize(&self) -> Result<(), AppError> {
        // 初始化 Telegram 连接
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<ChannelMessage>, AppError> {
        // 从 Telegram 接收消息
        Ok(Vec::new())
    }

    async fn send_message(
        &self,
        _channel_user_id: &str,
        content: &str,
    ) -> Result<ChannelMessageResponse, AppError> {
        // 发送消息到 Telegram
        Ok(ChannelMessageResponse {
            channel_message_id: None,
            content: content.to_string(),
            status: "sent".to_string(),
        })
    }

    async fn handle_command(
        &self,
        _channel_user_id: &str,
        command: &str,
        _args: Vec<String>,
    ) -> Result<String, AppError> {
        Ok(format!("Command {} executed", command))
    }

    async fn get_user_info(&self, channel_user_id: &str) -> Result<Value, AppError> {
        Ok(serde_json::json!({
            "channel_user_id": channel_user_id,
            "channel": "telegram"
        }))
    }

    async fn health_check(&self) -> Result<bool, AppError> {
        Ok(true)
    }
}

/// Web 适配器
pub struct WebAdapter;

impl WebAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ChannelAdapter for WebAdapter {
    fn channel_type(&self) -> &str {
        "web"
    }

    async fn initialize(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<ChannelMessage>, AppError> {
        Ok(Vec::new())
    }

    async fn send_message(
        &self,
        _channel_user_id: &str,
        content: &str,
    ) -> Result<ChannelMessageResponse, AppError> {
        Ok(ChannelMessageResponse {
            channel_message_id: None,
            content: content.to_string(),
            status: "sent".to_string(),
        })
    }

    async fn handle_command(
        &self,
        _channel_user_id: &str,
        command: &str,
        _args: Vec<String>,
    ) -> Result<String, AppError> {
        Ok(format!("Command {} executed", command))
    }

    async fn get_user_info(&self, channel_user_id: &str) -> Result<Value, AppError> {
        Ok(serde_json::json!({
            "channel_user_id": channel_user_id,
            "channel": "web"
        }))
    }

    async fn health_check(&self) -> Result<bool, AppError> {
        Ok(true)
    }
}
