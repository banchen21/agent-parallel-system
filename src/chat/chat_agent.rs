use actix::prelude::*;
use anyhow::Result;
use chrono::Utc;
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::{chat, task_handler::task_model::MessageClassificationResponse};

/// ChatAgent Actor结构体
pub struct ChatAgent {
    openai_client: async_openai::Client<async_openai::config::OpenAIConfig>,
    channel_manager: Addr<crate::channel::actor_manager::ChannelManagerActor>,
    prompt_template: String,
    config: ChatAgentConfig,
}

/// 流式请求类型
#[derive(Debug, Clone)]
pub struct StreamRequest {
    pub user: String,
    pub content: String,
    pub session_id: Option<String>,
}

/// ChatAgent配置
#[derive(Debug, Clone)]
pub struct ChatAgentConfig {
    pub model: String,
    pub max_tokens: u32,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub personality_path: String,
}

impl Default for ChatAgentConfig {
    fn default() -> Self {
        Self {
            model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
            max_tokens: 512,
            timeout_seconds: std::env::var("OPENAI_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(120), // 默认120秒，可通过环境变量配置
            max_retries: 3,
            personality_path: "config/personality_setting.md".to_string(),
        }
    }
}

impl ChatAgent {
    pub fn new(
        channel_manager: Addr<crate::channel::actor_manager::ChannelManagerActor>,
        openai_config: async_openai::config::OpenAIConfig,
        prompt_template: String,
    ) -> Self {
        Self {
            openai_client: async_openai::Client::with_config(openai_config),
            channel_manager,
            prompt_template,
            config: ChatAgentConfig::default(),
        }
    }

    /// 读取人格设定文件
    async fn load_personality_setting(&self) -> String {
        tokio::task::spawn_blocking({
            let path = self.config.personality_path.clone();
            move || {
                std::fs::read_to_string(&path).unwrap_or_else(|e| {
                    warn!("读取人格设定文件失败 ({}): {}, 使用默认值", path, e);
                    "你是一个智能的聊天助手，能够理解用户的意图并提供有用的回答。".to_string()
                })
            }
        })
        .await
        .unwrap_or_else(|e| {
            error!("异步任务失败: {}", e);
            "你是一个智能的聊天助手".to_string()
        })
    }

    /// 构建提示词
    fn build_prompt(&self, personality: &str, memory: &str, user_input: &str) -> String {
        self.prompt_template
            .replace(
                "{personality_setting}",
                &format!("【人格设定】\n{}", personality),
            )
            // TODO: 记忆压缩
            .replace("{memory_content}", &format!("\n【本地记忆】\n{}", memory))
            .replace("{user_input}", &format!("【用户内容】\n{}", user_input))
    }

    /// 调用OpenAI API（带重试）
    async fn call_openai_with_retry(&self, prompt: String) -> Result<String, ChatAgentError> {
        let client = self.openai_client.clone();
        let model = self.config.model.clone();
        let max_tokens = self.config.max_tokens;
        let timeout_secs = self.config.timeout_seconds;

        // 手动实现重试逻辑
        let mut retries = 0;
        let max_retries = self.config.max_retries;

        loop {
            let request = async_openai::types::chat::CreateChatCompletionRequestArgs::default()
                .model(model.clone())
                .max_tokens(max_tokens)
                .messages([
                    async_openai::types::chat::ChatCompletionRequestUserMessageArgs::default()
                        .content(prompt.clone())
                        .build()?
                        .into(),
                ])
                .build()?;

            match timeout(
                Duration::from_secs(timeout_secs),
                client.chat().create(request),
            )
            .await
            {
                Ok(Ok(response)) => {
                    if let Some(choice) = response.choices.first() {
                        if let Some(content) = &choice.message.content {
                            return Ok(content.clone());
                        }
                    }
                    return Err(ChatAgentError::OpenAIError(
                        async_openai::error::OpenAIError::InvalidArgument("空响应".to_string()),
                    ));
                }
                Ok(Err(e)) => {
                    error!("OpenAI调用失败: {}", e);
                    retries += 1;
                    if retries >= max_retries {
                        return Err(ChatAgentError::OpenAIError(e));
                    }
                    // 指数退避等待
                    let wait_time = Duration::from_secs(2u64.pow(retries));
                    tokio::time::sleep(wait_time).await;
                }
                Err(_) => {
                    error!("OpenAI调用超时");
                    return Err(ChatAgentError::TimeoutError("超时".to_string()));
                }
            }
        }
    }

    /// 处理分类响应
    fn handle_classification(&self, content: String) -> Result<ChatAgentResponse, ChatAgentError> {
        match MessageClassificationResponse::from_json(&content) {
            Ok(classification) => {
                debug!("预览消息:{:#?}", classification);
                let response_message = chat::model::UserMessage {
                    user: "ChatAgent".to_string(),
                    content: chat::model::MessageContent::Text(
                        classification.clone().content,
                    ),
                    message_type: chat::model::MessageType::Chat,
                    source_ip: "local".to_string(),
                    metadata: HashMap::new(),
                    created_at: Utc::now(),
                };
                if classification.has_tasks() {
                    //TODO: 任务处理
                    info!(
                        "检测到{}个任务，转发给任务处理器",
                        classification.task_count()
                    );
                }
                Ok(ChatAgentResponse {
                    content: response_message,
                })
            }
            Err(e) => {
                error!("解析失败: {}, 原始内容: {}", e, content);
                // 重试逻辑
                Err(ChatAgentError::SerializationError(e))
            }
        }
    }
}

impl Actor for ChatAgent {
    type Context = Context<Self>;
}

/// 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAgentResponse {
    pub content: chat::model::UserMessage,
}

#[derive(Message)]
#[rtype(result = "Result<ChatAgentResponse, ChatAgentError>")]
pub struct OtherUserMessage {
    pub content: chat::model::UserMessage,
}

/// 简化的异步Handler实现
impl Handler<OtherUserMessage> for ChatAgent {
    type Result = ResponseFuture<Result<ChatAgentResponse, ChatAgentError>>;

    fn handle(&mut self, msg: OtherUserMessage, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let user_input = msg.content.content.to_string();

        Box::pin(async move {
            debug!(
                "📨 开始处理消息: {}",
                user_input.chars().take(50).collect::<String>()
            );

            // 1. 加载人格设定
            let personality = this.load_personality_setting().await;

            // 2. 构建提示词
            let prompt = this.build_prompt(&personality, "", &user_input);

            // 3. 调用OpenAI（带重试）
            let response = match this.call_openai_with_retry(prompt).await {
                Ok(r) => r,
                Err(e) => {
                    error!("OpenAI调用最终失败: {}", e);
                    return Err(e);
                }
            };

            // 4. 处理响应
            this.handle_classification(response)
        })
    }
}

impl Clone for ChatAgent {
    fn clone(&self) -> Self {
        Self {
            openai_client: self.openai_client.clone(),
            channel_manager: self.channel_manager.clone(),
            prompt_template: self.prompt_template.clone(),
            config: self.config.clone(),
        }
    }
}

/// 错误类型
#[derive(Debug, Error)]
pub enum ChatAgentError {
    #[error("OpenAI API 调用失败: {0}")]
    OpenAIError(#[from] async_openai::error::OpenAIError),

    #[error("处理超时: {0}")]
    TimeoutError(String),

    #[error("序列化错误: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("任务加入失败: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("重试失败: {0}")]
    RetryError(String),
}
