use crate::chat::actor_messages::{ChannelManagerActor, GetMessages, ResultMessage};
use crate::chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor};
use crate::graph_memory::actor_memory::GetMyName;
use crate::graph_memory::actor_memory::{AgentMemoryHActor, RequestMemory};
use crate::task_handler::task_agent::{OtherMessage, TaskAgent};
use crate::utils::json_util::clean_json_string;
use crate::{chat, task_handler::task_model::MessageClassificationResponse};
use actix::prelude::*;
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

pub const PERSONALITY_PATH: &str = "config/personality_setting.md";

/// ChatAgent Actor结构体
pub struct ChatAgent {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    channel_manager: Addr<ChannelManagerActor>,
    agent_memory_hactor: Addr<AgentMemoryHActor>,
    task_agent: Addr<TaskAgent>,
    prompt: String,
}
impl ChatAgent {
    pub fn new(
        channel_manager: Addr<ChannelManagerActor>,
        neo4j_channel_manager: Addr<AgentMemoryHActor>,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        task_agent: Addr<TaskAgent>,
        prompt: String,
    ) -> Self {
        Self {
            channel_manager,
            agent_memory_hactor: neo4j_channel_manager,
            open_aiproxy_actor,
            task_agent,
            prompt,
        }
    }

    /// 读取人格设定文件
    async fn load_personality_setting(&self) -> String {
        tokio::task::spawn_blocking({
            let path = PERSONALITY_PATH.clone();
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
    fn build_prompt(
        &self,
        personality_setting: String,
        knowledge_summary: String,
        chat_history: Vec<ResultMessage>,
        user_name: String,
        user_content: String,
        is_task: bool,
    ) -> String {
        self.prompt
            .replace("{personality_setting}", &format!("{}", personality_setting))
            .replace("{knowledge_summary}", &format!("{}", knowledge_summary))
            .replace("{chat_history}", &format!("{:?}", chat_history))
            .replace("{user_name}", &format!("{}", user_name))
            .replace("{user_content}", &format!("{:?}", user_content))
            .replace("{is_task}", &format!("{}", is_task))
    }
}

impl Actor for ChatAgent {
    type Context = Context<Self>;
}

/// 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAgentResponse {
    pub content: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: String,
    pub created_at: DateTime<Utc>,
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
        let user_message: chat::model::UserMessage = msg.content.clone();

        Box::pin(async move {
            // 提示词模板
            let personality_task = this.load_personality_setting().await;
            let ai_name = this.agent_memory_hactor.send(GetMyName {}).await.unwrap();
            // 获取短期记忆（聊天记录）
            let momory_content_short: Vec<ResultMessage> = match this
                .channel_manager
                .send(GetMessages {
                    user: user_message.sender.clone(),
                    ai_name,
                    before: Some(msg.content.created_at),
                    limit: 10,
                })
                .await
            {
                Ok(dd) => match dd {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        error!("❌ 获取消息历史失败: {}", e);
                        vec![]
                    }
                },
                Err(_) => {
                    vec![]
                }
            };

            // 获取长期记忆
            let memory_mailbox_res = this
                .agent_memory_hactor
                .send(RequestMemory {
                    user_name: user_message.sender.clone(),
                    // 短期记忆
                    momory_content_short: momory_content_short.clone(),
                    message_content: user_message.content.clone(),
                })
                .await;
            let memory_content = match memory_mailbox_res {
                Ok(inner_res) => match inner_res {
                    Ok(content) => content,
                    Err(e) => {
                        warn!("⚠️ 记忆智能体业务处理失败: {}", e);
                        "（对该用户暂无相关历史记忆背景）".to_string()
                    }
                },
                Err(e) => {
                    error!("❌ 记忆智能体通信失败 (MailboxError): {}", e);
                    "（认知系统离线，无法获取记忆背景）".to_string()
                }
            };

            // 提交给意图识别智能体
            let res_task_message_classification = this
                .task_agent
                .send(OtherMessage {
                    long_term_memory: memory_content.clone(),
                    short_term_memory: momory_content_short.clone(),
                    user_content: user_message.content.clone(),
                })
                .await;
            debug!(
                "意图识别智能体返回结果: {:#?}",
                res_task_message_classification
            );
            let is_task = match res_task_message_classification {
                Ok(inner_res) => match inner_res {
                    Ok(content) => content.is_task,
                    Err(e) => {
                        error!("❌ 意图识别智能体业务处理失败: {}", e);
                        false
                    }
                },
                Err(e) => {
                    error!("❌ 意图识别智能体通信失败:{}", e);
                    false
                }
            };

            // 提示词构建
            let prompt = this.build_prompt(
                personality_task,
                memory_content,
                momory_content_short,
                user_message.sender.clone(),
                user_message.content.to_string(),
                is_task,
            );

            let response_text = this
                .open_aiproxy_actor
                .send(CallOpenAI {
                    chat_completion_request_message: vec![
                        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(prompt),
                            name: None,
                        }),
                        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                            name: Some(user_message.sender),
                            content: ChatCompletionRequestUserMessageContent::Text(
                                user_message.content.to_string(),
                            ),
                        }),
                    ],
                })
                .await
                .map_err(ChatAgentError::from)??;

            // 5. 处理响应结果
            let response_text = clean_json_string(&response_text).to_string();
            debug!("🎉 ChatAgent 响应消息: {}", response_text);
            #[derive(Debug, Serialize, Deserialize)]
            struct ContentItem {
                text: String,
            }
            #[derive(Debug, Serialize, Deserialize)]
            struct MessagePayload {
                content: Vec<ContentItem>,
            }
            let mut chat_messages_list = vec![];
            match serde_json::from_str::<MessagePayload>(&response_text) {
                Ok(parsed_data) => {
                    // 遍历并提取里面的具体文本
                    for (_, item) in parsed_data.content.iter().enumerate() {
                        chat_messages_list.push(ChatMessage {
                            content: item.text.clone(),
                            created_at: Utc::now(),
                        });
                    }
                }
                Err(e) => {
                    error!("❌ 解析 JSON 失败: {}", e);
                    // 重试逻辑
                    return Err(ChatAgentError::LogicError(format!(
                        "JSON解析失败: {}, 原始响应: {}",
                        e, response_text
                    )));
                }
            }

            Ok(ChatAgentResponse {
                content: chat_messages_list,
            })
        })
    }
}

impl Clone for ChatAgent {
    fn clone(&self) -> Self {
        Self {
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            channel_manager: self.channel_manager.clone(),
            agent_memory_hactor: self.agent_memory_hactor.clone(),
            task_agent: self.task_agent.clone(),
            prompt: self.prompt.clone(),
        }
    }
}
