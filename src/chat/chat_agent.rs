use actix::prelude::*;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::chat::actor_messages::ChannelManagerActor;
use crate::chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor};
use crate::graph_memory::actor_memory::{AgentMemoryHActor, RequestMemory};
use crate::utils::json_util::clean_json_string;
use crate::{chat, task_handler::task_model::MessageClassificationResponse};

/// ChatAgent Actor结构体
pub struct ChatAgent {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    channel_manager: Addr<ChannelManagerActor>,
    agent_memory_manager: Addr<AgentMemoryHActor>,
    prompt_template: String,
    config: ChatAgentConfig,
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
        channel_manager: Addr<ChannelManagerActor>,
        neo4j_channel_manager: Addr<AgentMemoryHActor>,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        prompt_template: String,
    ) -> Self {
        Self {
            channel_manager,
            agent_memory_manager: neo4j_channel_manager,
            open_aiproxy_actor,
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

    /// 处理分类响应
    fn handle_classification(&self, content: String) -> Result<ChatAgentResponse, ChatAgentError> {
        match MessageClassificationResponse::from_json(&content) {
            Ok(classification) => {
                let response_message = chat::model::UserMessage {
                    user: "ChatAgent".to_string(),
                    content: chat::model::MessageContent::Text(classification.clone().content),
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
        let memory_manager = self.agent_memory_manager.clone();
        let openai_actor = self.open_aiproxy_actor.clone();
        let user_name = msg.content.user.clone();

        Box::pin(async move {
            debug!(
                "📨 开始处理消息: 用户={}, 内容={}",
                user_name,
                user_input.chars().take(20).collect::<String>()
            );

            // 1 & 2. 【优化】并行加载人格设定和本地记忆
            // 使用 tokio::join! 让两个耗时操作同时进行
            let personality_task = this.load_personality_setting();

            let memory_task = memory_manager.send(RequestMemory {
                user_name: user_name.clone(),
                message_content: user_input.clone(),
            });

            let (personality, memory_mailbox_res) = tokio::join!(personality_task, memory_task);

            // 处理记忆返回结果
            let memory_content = match memory_mailbox_res {
                Ok(inner_res) => match inner_res {
                    Ok(content) => content,
                    Err(e) => {
                        warn!("⚠️ 记忆智能体业务处理失败: {}", e);
                        "（该用户暂无相关历史记忆背景）".to_string()
                    }
                },
                Err(e) => {
                    error!("❌ 记忆智能体通信失败 (MailboxError): {}", e);
                    "（认知系统离线，无法获取记忆背景）".to_string()
                }
            };

            // 构建提示词
            let prompt = this.build_prompt(&personality, &memory_content, &user_input);
            let response_text = openai_actor
                .send(CallOpenAI { prompt })
                .await
                .map_err(ChatAgentError::from)??;

            // 5. 处理响应结果

            this.handle_classification(clean_json_string(&response_text).to_string())
        })
    }
}

impl Clone for ChatAgent {
    fn clone(&self) -> Self {
        Self {
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            channel_manager: self.channel_manager.clone(),
            agent_memory_manager: self.agent_memory_manager.clone(),
            prompt_template: self.prompt_template.clone(),
            config: self.config.clone(),
        }
    }
}
