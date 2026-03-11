use actix::prelude::*;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::ChatCompletionRequestMessage;
use std::time::Duration;
use thiserror::Error;
use tokio::time::timeout;
use tracing::{debug, error, warn};
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
}
impl Handler<CallOpenAI> for OpenAIProxyActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: CallOpenAI, _ctx: &mut Self::Context) -> Self::Result {
        let client = self.client.clone();
        let model = self.model.clone();
        let max_tokens = self.max_tokens;
        let timeout_secs = self.timeout_secs;
        let max_retries = self.max_retries;
        let prompt = msg.chat_completion_request_message;

        let fut = async move {
            let mut last_err = ChatAgentError::TimeoutError("初始状态".into());

            // 使用 for 循环更清晰地控制重试次数
            for attempt in 1..=max_retries {
                let request_res =
                    async_openai::types::chat::CreateChatCompletionRequestArgs::default()
                        .model(&model)
                        .max_tokens(max_tokens)
                        .messages(prompt.clone())
                        .build()
                        .map_err(ChatAgentError::from);

                let request = match request_res {
                    Ok(r) => r,
                    Err(e) => return Err(e),
                };

                // 执行请求并带超时控制
                match timeout(
                    Duration::from_secs(timeout_secs),
                    client.chat().create(request),
                )
                .await
                {
                    // 1. 请求成功返回
                    Ok(Ok(response)) => {
                        if let Some(choice) = response.choices.first() {
                            if let Some(content) = &choice.message.content {
                                return Ok(content.clone());
                            }
                        }
                        return Err(ChatAgentError::TimeoutError("API返回了空响应".into()));
                    }

                    // 2. API 报错或网络错误 (例如你遇到的 UnexpectedEof)
                    Ok(Err(e)) => {
                        warn!(
                            "OpenAI 调用出错 (第 {}/{} 次尝试): {}",
                            attempt, max_retries, e
                        );
                        last_err = ChatAgentError::OpenAIError(e);
                    }

                    // 3. 响应超时
                    Err(_) => {
                        warn!("OpenAI 调用超时 (第 {}/{} 次尝试)", attempt, max_retries);
                        last_err = ChatAgentError::TimeoutError("OpenAI调用超时".into());
                    }
                }

                // 如果还没到最后一次尝试，则等待后重试
                if attempt < max_retries {
                    let backoff = Duration::from_secs(2u64.pow(attempt as u32));
                    debug!("等待 {:?} 后进行下一次重试...", backoff);
                    tokio::time::sleep(backoff).await;
                }
            }

            // 耗尽重试次数后返回最后的错误
            error!("OpenAI 调用在 {} 次重试后仍然失败", max_retries);
            Err(last_err)
        };

        Box::pin(fut.into_actor(self))
    }
}
