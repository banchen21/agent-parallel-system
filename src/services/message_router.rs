use sqlx::PgPool;
use uuid::Uuid;
use tracing::info;

use crate::{
    core::errors::AppError,
    models::chat::MessageRole,
    services::{
        ChatService, ChannelService, LLMClient,
        channel_adapter::{ChannelMessage, ChannelAdapterFactory},
        llm_client::ChatCompletionMessage,
    },
};

/// 消息路由服务
pub struct MessageRouterService {
    db_pool: PgPool,
    chat_service: ChatService,
    channel_service: ChannelService,
}

impl MessageRouterService {
    pub fn new(
        db_pool: PgPool,
        chat_service: ChatService,
        channel_service: ChannelService,
    ) -> Self {
        Self {
            db_pool,
            chat_service,
            channel_service,
        }
    }

    /// 处理来自通道的消息
    pub async fn handle_channel_message(
        &self,
        channel_config_id: Uuid,
        message: ChannelMessage,
        llm_client: &LLMClient,
    ) -> Result<String, AppError> {
        info!("处理通道消息: {:?}", message);

        // 1. 获取或创建通道用户
        let channel_user = self
            .channel_service
            .get_or_create_channel_user(
                channel_config_id,
                &message.channel_user_id,
                message.channel_username.as_deref(),
            )
            .await?;

        // 2. 获取或创建全局聊天会话
        let session = self
            .chat_service
            .get_or_create_global_session(channel_user.id)
            .await?;

        // 3. 保存用户消息
        self.chat_service
            .add_message(session.id, MessageRole::User.as_str(), &message.content, None)
            .await?;

        // 4. 获取消息历史
        let messages = self
            .chat_service
            .get_session_messages(session.id, session.context_window as i64)
            .await?;

        // 5. 构建 LLM 请求
        let mut llm_messages = Vec::new();
        if let Some(system_prompt) = &session.system_prompt {
            llm_messages.push(ChatCompletionMessage {
                role: MessageRole::System.to_string(),
                content: system_prompt.clone(),
            });
        }

        for msg in messages.iter().rev() {
            llm_messages.push(ChatCompletionMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        // 6. 调用 LLM
        let (response_content, tokens_used) = llm_client
            .get_response(llm_messages, session.temperature, session.max_tokens)
            .await?;

        // 7. 保存助手回复
        self.chat_service
            .add_message(
                session.id,
                MessageRole::Assistant.as_str(),
                &response_content,
                tokens_used,
            )
            .await?;

        info!("消息处理完成，回复: {}", response_content);

        Ok(response_content)
    }

    /// 处理命令
    pub async fn handle_command(
        &self,
        channel_config_id: Uuid,
        channel_user_id: &str,
        command: &str,
        _args: Vec<String>,
    ) -> Result<String, AppError> {
        match command {
            "new" => {
                // 创建新会话
                let channel_user = self
                    .channel_service
                    .get_or_create_channel_user(channel_config_id, channel_user_id, None)
                    .await?;

                self.chat_service
                    .create_session(crate::models::chat::CreateChatSessionRequest {
                        channel_user_id: channel_user.id,
                        title: Some("New Chat Session".to_string()),
                        model: None,
                        system_prompt: None,
                        temperature: None,
                        max_tokens: None,
                        context_window: None,
                    })
                    .await?;

                Ok("新会话已创建".to_string())
            }
            "help" => {
                Ok("可用命令:\n/new - 创建新会话\n/help - 显示帮助".to_string())
            }
            _ => Err(AppError::ValidationError(format!("未知命令: {}", command))),
        }
    }
}
