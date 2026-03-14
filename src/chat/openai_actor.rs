use actix::prelude::*;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionToolChoiceOption, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, types::chat::ChatCompletionTools};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, warn};

use thiserror::Error;

/// 错误类型
#[derive(Debug, Error)]
pub enum ChatAgentError {
    #[error("OpenAI API 调用失败: {0}")]
    OpenAIError(#[from] async_openai::error::OpenAIError),

    #[error("Actor 通信失败: {0}")]
    MailboxError(#[from] actix::MailboxError),

    #[error("处理超时: {0}")]
    TimeoutError(String),

    #[error("序列化错误: {0}")]
    SerializationError(#[from] serde_json::Error),

    // UnexpectedResponse
    #[error("意外响应: {0}")]
    UnexpectedResponse(String),

    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("图数据库错误: {0}")]
    GraphError(#[from] neo4rs::Error),

    #[error("任务加入失败: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("逻辑错误: {0}")]
    LogicError(String),

    // 查询错误
    #[error("查询错误: {0}")]
    QueryError(String),
}

pub struct OpenAIProxyActor {
    client: Client<OpenAIConfig>,
    model: String,
    max_tokens: u32,
    timeout_secs: u64,
    max_retries: u32,
}

impl OpenAIProxyActor {
    pub fn new(config: OpenAIConfig, model: String, timeout_secs: u64, max_tokens: u32) -> Self {
        Self {
            client: Client::with_config(config),
            model,
            max_tokens,
            timeout_secs,
            max_retries: 3,
        }
    }
}

impl Actor for OpenAIProxyActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct CallOpenAI {
    pub chat_completion_request_message: Vec<ChatCompletionRequestMessage>,
    pub tools: Option<Vec<ChatCompletionTools>>,
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
}

impl Handler<CallOpenAI> for OpenAIProxyActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: CallOpenAI, _ctx: &mut Self::Context) -> Self::Result {
        let client = self.client.clone();
        let model = self.model.clone();
        let max_tokens = self.max_tokens;
        let timeout_secs = self.timeout_secs;
        let max_retries = self.max_retries;
        let messages = msg.chat_completion_request_message;

        let fut = async move {
            let mut last_err = ChatAgentError::TimeoutError("初始状态".into());

            for attempt in 1..=max_retries {
                // 1. 创建 Builder
                let mut builder = CreateChatCompletionRequestArgs::default();
                builder.model(&model);
                builder.max_tokens(max_tokens);
                builder.messages(messages.clone());

                if let Some(tools) = msg.tools.clone() {
                    if !tools.is_empty() {
                        builder.tools(tools);
                    }
                }

                if let Some(tool_choice) = msg.tool_choice.clone() {
                    builder.tool_choice(tool_choice);
                }

                // 2. 构建请求
                let request = match builder.build() {
                    Ok(req) => req,
                    Err(e) => return Err(ChatAgentError::from(e)),
                };
                // 3. 发起 API 调用，带超时
                match timeout(
                    Duration::from_secs(timeout_secs),
                    client.chat().create(request),
                )
                .await
                {
                    Ok(Ok(response)) => {
                        if let Some(choice) = response.choices.first() {
                            // 优先返回文本
                            if let Some(content) = &choice.message.content {
                                return Ok(content.clone());
                            }

                            // 如果有工具调用，则返回 JSON
                            if let Some(tool_calls) = &choice.message.tool_calls {
                                match serde_json::to_string(tool_calls) {
                                    Ok(json) => return Ok(json),
                                    Err(e) => return Err(ChatAgentError::SerializationError(e)),
                                }
                            }

                            return Err(ChatAgentError::UnexpectedResponse(
                                "模型返回了空内容且无工具调用".into(),
                            ));
                        } else {
                            eprintln!("API响应choices为空，完整响应: {:#?}", response);
                            return Err(ChatAgentError::UnexpectedResponse(
                                "API 返回的 choices 为空".into(),
                            ));
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "OpenAI 调用出错 (第 {}/{} 次尝试): {}",
                            attempt, max_retries, e
                        );
                        last_err = ChatAgentError::OpenAIError(e);
                    }
                    Err(_) => {
                        warn!("OpenAI 调用超时 (第 {}/{} 次尝试)", attempt, max_retries);
                        last_err = ChatAgentError::TimeoutError("OpenAI调用超时".into());
                    }
                }

                // 4. 重试前等待（指数退避）
                if attempt < max_retries {
                    let backoff = Duration::from_secs(2u64.pow(attempt as u32));
                    debug!("等待 {:?} 后进行下一次重试...", backoff);
                    tokio::time::sleep(backoff).await;
                }
            }

            error!("OpenAI 调用在 {} 次重试后仍然失败", max_retries);
            Err(last_err)
        };

        Box::pin(fut.into_actor(self))
    }
}
