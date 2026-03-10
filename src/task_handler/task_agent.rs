use actix::prelude::*;
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::chat::actor_messages::{ChannelManagerActor, GetMessages, ResultMessage};
use crate::chat::model::MessageContent;
use crate::chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor};
use crate::graph_memory::actor_memory::{AgentMemoryHActor, RequestMemory};
use crate::utils::json_util::clean_json_string;
use crate::{chat, task_handler::task_model::MessageClassificationResponse};

/// TaskAgent Actor结构体
/// 处理意图（指令）分析
pub struct TaskAgent {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    prompt: String, // 提示词
}

impl Actor for TaskAgent {
    type Context = Context<Self>;
}

impl TaskAgent {
    pub fn new(open_aiproxy_actor: Addr<OpenAIProxyActor>, prompt: String) -> Self {
        Self {
            open_aiproxy_actor,
            prompt,
        }
    }

    // 提示词构建
    fn build_prompt(
        &self,
        long_term_memory: &str,
        short_term_memory: Vec<ResultMessage>,
        user_content: MessageContent,
    ) -> String {
        self.prompt
            .clone()
            .replace("{memory_content}", long_term_memory)
            .replace(
                "{momory_content_short}",
                &format!("{:#?}", short_term_memory),
            )
    }

    /// 处理分类响应
    fn handle_classification(
        &self,
        content: String,
    ) -> Result<MessageClassificationResponse, ChatAgentError> {
        match MessageClassificationResponse::from_json(clean_json_string(&content)) {
            Ok(classification) => Ok(classification),
            Err(e) => {
                error!("解析失败: {}, 原始内容: {}", e, content);
                // 重试逻辑
                Err(ChatAgentError::SerializationError(e))
            }
        }
    }
}

// 分类
#[derive(Message)]
#[rtype(result = "Result<MessageClassificationResponse, ChatAgentError>")]
pub struct OtherMessage {
    // 长期记忆
    pub long_term_memory: String,
    // 短期记忆
    pub short_term_memory: Vec<ResultMessage>,
    // 用户内容
    pub user_content: MessageContent,
}

/// 简化的异步Handler实现
impl Handler<OtherMessage> for TaskAgent {
    type Result = ResponseFuture<Result<MessageClassificationResponse, ChatAgentError>>;

    fn handle(&mut self, msg: OtherMessage, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();

        Box::pin(async move {
            // 1. 获取长期记忆
            let long_term_memory = msg.long_term_memory;
            // 2. 获取短期记忆
            let short_term_memory = msg.short_term_memory;
            // 4. 获取用户内容
            let user_content: MessageContent = msg.user_content;

            // 提示词构建
            let prompt =
                this.build_prompt(&long_term_memory, short_term_memory, user_content.clone());

            // 4. 发送请求
            let response_text = this
                .open_aiproxy_actor
                .send(CallOpenAI {
                    chat_completion_request_message: vec![
                        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(prompt),
                            name: None,
                        }),
                        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                            name: None,
                            content: ChatCompletionRequestUserMessageContent::Text(
                                user_content.to_string(),
                            ),
                        }),
                    ],
                })
                .await
                .map_err(ChatAgentError::from)??;

            // 5. 处理响应结果
            this.handle_classification(response_text)
        })
    }
}

impl Clone for TaskAgent {
    fn clone(&self) -> Self {
        Self {
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            prompt: self.prompt.clone(),
        }
    }
}
