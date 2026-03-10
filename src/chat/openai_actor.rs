use actix::prelude::*;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::ChatCompletionRequestMessage;
use std::time::Duration;
use thiserror::Error;
use tokio::time::timeout;
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

     #[error("内部逻辑错误: {0}")]
    InternalError(String),

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
    // 确保这里的 Result 类型与 ActorFuture 的输出一致
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: CallOpenAI, _ctx: &mut Self::Context) -> Self::Result {
        // 1. 克隆所有权变量
        let client = self.client.clone();
        let model = self.model.clone();
        let max_tokens = self.max_tokens;
        let timeout_secs = self.timeout_secs;
        let max_retries = self.max_retries;
        let prompt = msg.chat_completion_request_message;

        // 2. 显式标注异步块的返回类型，解决 {unknown} 问题
        let fut = async move {
            let mut retries = 0;
            loop {
                // 构造请求
                let request_res = async_openai::types::chat::CreateChatCompletionRequestArgs::default()
                    .model(&model)
                    .max_tokens(max_tokens)
                    .messages(prompt.clone())
                    .build()
                    .map_err(ChatAgentError::from);

                let request = match request_res {
                    Ok(r) => r,
                    Err(e) => return Err(e),
                };

                // 执行请求并带超时
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
                        return Err(ChatAgentError::TimeoutError("空响应".into()));
                    }
                    Ok(Err(e)) => {
                        retries += 1;
                        if retries >= max_retries {
                            return Err(ChatAgentError::OpenAIError(e));
                        }
                        // 指数退避
                        tokio::time::sleep(Duration::from_secs(2u64.pow(retries))).await;
                    }
                    Err(_) => {
                        return Err(ChatAgentError::TimeoutError("OpenAI调用超时".into()));
                    }
                }
            }
        };

        // 3. 关键修复：使用 wrap_future 并显式转换成 Box::pin 的特质对象
        Box::pin(fut.into_actor(self))
    }
}
