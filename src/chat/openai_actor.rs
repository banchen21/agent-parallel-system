use actix::prelude::*;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionToolChoiceOption,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, types::chat::ChatCompletionTools};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

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

    #[error("代理商不存在: {0}")]
    ProviderNotFound(String),
}

// ======================== 代理商配置 ========================

/// 单个代理商的配置（用于注册或初始化）
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// 代理商名称（唯一键），例如 "openai"、"deepseek"、"ollama"
    pub name: String,
    /// API Key
    pub api_key: String,
    /// API Base URL（OpenAI 兼容接口）
    pub base_url: String,
    /// 该代理商下的默认模型
    pub default_model: String,
}

impl ProviderConfig {
    pub fn new(
        name: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            default_model: default_model.into(),
        }
    }

    fn build_client(&self) -> Client<OpenAIConfig> {
        let cfg = OpenAIConfig::default()
            .with_api_key(&self.api_key)
            .with_api_base(&self.base_url);
        Client::with_config(cfg)
    }
}

// ======================== Actor ========================

struct ProviderEntry {
    client: Client<OpenAIConfig>,
    default_model: String,
}

pub struct OpenAIProxyActor {
    providers: HashMap<String, ProviderEntry>,
    default_provider: String,
    max_tokens: u32,
    timeout_secs: u64,
    max_retries: u32,
}

impl OpenAIProxyActor {
    pub fn new(
        default: ProviderConfig,
        timeout_secs: u64,
        max_tokens: u32,
    ) -> Self {
        let mut providers = HashMap::new();
        let name = default.name.clone();
        providers.insert(
            name.clone(),
            ProviderEntry {
                client: default.build_client(),
                default_model: default.default_model,
            },
        );
        Self {
            providers,
            default_provider: name,
            max_tokens,
            timeout_secs,
            max_retries: 3,
        }
    }

    /// 添加额外的代理商（链式，方便 builder 风格）
    pub fn with_provider(mut self, cfg: ProviderConfig) -> Self {
        self.providers.insert(
            cfg.name.clone(),
            ProviderEntry {
                client: cfg.build_client(),
                default_model: cfg.default_model,
            },
        );
        self
    }
}

impl Actor for OpenAIProxyActor {
    type Context = Context<Self>;
}
// ======================== 消息：调用 LLM ========================

#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct CallOpenAI {
    pub chat_completion_request_message: Vec<ChatCompletionRequestMessage>,
    pub tools: Option<Vec<ChatCompletionTools>>,
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
    /// 指定代理商名称；None 表示使用默认代理商
    pub provider: Option<String>,
    /// 指定模型；None 表示使用该代理商的默认模型
    pub model: Option<String>,
}

impl Handler<CallOpenAI> for OpenAIProxyActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: CallOpenAI, _ctx: &mut Self::Context) -> Self::Result {
        // 选定代理商
        let provider_name = msg
            .provider
            .as_deref()
            .unwrap_or(&self.default_provider)
            .to_string();

        let entry = match self.providers.get(&provider_name) {
            Some(e) => e,
            None => {
                let err = ChatAgentError::ProviderNotFound(provider_name.clone());
                return Box::pin(async move { Err(err) }.into_actor(self));
            }
        };

        let client = entry.client.clone();
        // 优先使用消息中指定的模型，否则用代理商默认模型
        let model = msg
            .model
            .clone()
            .unwrap_or_else(|| entry.default_model.clone());

        let max_tokens = self.max_tokens;
        let timeout_secs = self.timeout_secs;
        let max_retries = self.max_retries;
        let messages = msg.chat_completion_request_message;

        let fut = async move {
            let mut last_err = ChatAgentError::TimeoutError("初始状态".into());

            for attempt in 1..=max_retries {
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

                let request = match builder.build() {
                    Ok(req) => req,
                    Err(e) => return Err(ChatAgentError::from(e)),
                };

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
                            return Err(ChatAgentError::UnexpectedResponse(
                                "API 返回的 choices 为空".into(),
                            ));
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "[{}] OpenAI 调用出错 (第 {}/{} 次): {}",
                            provider_name, attempt, max_retries, e
                        );
                        last_err = ChatAgentError::OpenAIError(e);
                    }
                    Err(_) => {
                        warn!(
                            "[{}] OpenAI 调用超时 (第 {}/{} 次)",
                            provider_name, attempt, max_retries
                        );
                        last_err = ChatAgentError::TimeoutError("OpenAI调用超时".into());
                    }
                }

                if attempt < max_retries {
                    let backoff = Duration::from_secs(2u64.pow(attempt as u32));
                    debug!("等待 {:?} 后进行下一次重试...", backoff);
                    tokio::time::sleep(backoff).await;
                }
            }

            error!("[{}] 调用在 {} 次重试后仍然失败", provider_name, max_retries);
            Err(last_err)
        };

        Box::pin(fut.into_actor(self))
    }
}

