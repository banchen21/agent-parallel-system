use actix::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use async_openai::types::chat::ChatCompletionRequestMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessageContent;
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::chat::actor_messages::ResultMessage;
use crate::chat::{
    chat_agent::PERSONALITY_PATH,
    model::MessageContent,
    openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor},
};
use crate::core::config::MemoryAgentConfig;
use crate::graph_memory::neo4j_model::Neo4jOperation;
use crate::graph_memory::neo4j_model::fetch_all_entities;
use crate::graph_memory::neo4j_model::format_graph_to_string;
use crate::utils::json_util::clean_json_string;

/// 从人格设定中提取的“本我”结构
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AssistantIdentity {
    pub name: String, // 名字，如“齐悦”
}

pub struct AgentMemoryActor {
    pub graph: Graph,
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
    // 状态：当前"本我"的识别名，初始默认为 Assistant
    pub ai_name: String,
    pub agent_memory_prompt: MemoryAgentConfig,
    pub enable_memory_query: bool,
}
impl AgentMemoryActor {
    async fn load_personality_file() -> String {
        let path = PERSONALITY_PATH.to_string();
        tokio::task::spawn_blocking(move || {
            std::fs::read_to_string(&path).unwrap_or_else(|_| "你是一个智能助手".to_string())
        })
        .await
        .unwrap_or_default()
    }

    pub async fn new(
        uri: &str,
        user: &str,
        pass: &str,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        agent_memory_prompt: MemoryAgentConfig,
        enable_memory_query: bool,
    ) -> Result<Self> {
        // 1. 初始化数据库连接
        let graph = Graph::new(uri, user, pass).await?;

        // 2. 初始化根节点（如果不存在则创建，默认名为“我”）
        Self::init_root_node(&graph).await?;

        // 3. 获取数据库中当前的根节点名字
        let current_db_name = Self::get_root_name(&graph).await?;

        let ai_name = if current_db_name != "我" {
            // 情况 A: 数据库里已经有名了（不是初始的“我”），直接使用，跳过 AI
            info!(
                "检测到数据库已存在人格名称 '{}'，跳过 AI 提取。",
                current_db_name
            );
            current_db_name
        } else {
            // 情况 B: 名字还是“我”，说明需要从人格设定文件中提取
            info!("检测到初始状态，正在从人格设定提取 AI 名字...");
            let raw_personality = Self::load_personality_file().await;

            match Self::extract_ai_name(&open_aiproxy_actor, &raw_personality).await {
                Ok(extracted_name) => {
                    // 提取成功，更新数据库
                    let _ = Self::update_root_name_static(&graph, &extracted_name).await;
                    extracted_name
                }
                Err(e) => {
                    warn!("从 AI 提取名字失败，将沿用默认值 '我': {}", e);
                    "我".to_string()
                }
            }
        };

        // 4. 构建实例
        Ok(Self {
            graph,
            open_aiproxy_actor,
            ai_name,
            agent_memory_prompt,
            enable_memory_query,
        })
    }

    async fn extract_ai_name(
        actor: &Addr<OpenAIProxyActor>,
        personality_text: &str,
    ) -> Result<String> {
        let extract_prompt = format!(
            "# 任务\n从以下【人格设定文本】中提取“名字”。\n要求仅输出 JSON：{{\"name\": \"名字\"}}\n\n# 【人格设定文本】：\n{}",
            personality_text
        );

        // 发送 Actor 消息并处理两层 Result
        let json_res = actor
            .send(CallOpenAI {
                chat_completion_request_message: vec![ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: ChatCompletionRequestUserMessageContent::Text(extract_prompt),
                        name: None,
                    },
                )],
                tools: None,
                tool_choice: None,
            })
            .await
            .map_err(|e| anyhow!("Mailbox Error: {}", e))??; // 第一层是 Actor 错误，第二层是 OpenAI 业务错误

        // 清洗并解析 JSON
        let cleaned_json = clean_json_string(&json_res);
        let identity: AssistantIdentity = serde_json::from_str(&cleaned_json).map_err(|e| {
            anyhow!(
                "解析 AI 返回的 JSON 失败: {}, 原始字符串: {}",
                e,
                cleaned_json
            )
        })?;

        Ok(identity.name)
    }

    async fn init_root_node(graph: &Graph) -> Result<()> {
        let check_query = "MATCH (r:root) RETURN count(r) as exists";
        let mut result = graph.execute(query(check_query)).await?;

        let count: i64 = if let Ok(Some(row)) = result.next().await {
            row.get("exists").unwrap_or(0)
        } else {
            0
        };

        if count == 0 {
            graph
                .run(query(
                    "CREATE (r:root { name: '我', created_at: datetime() })",
                ))
                .await?;
            info!("数据库中无根节点，初始化创建‘我’成功！");
        }
        Ok(())
    }

    /// 新增：从数据库获取当前 root 的名字
    async fn get_root_name(graph: &Graph) -> Result<String> {
        let name_query = "MATCH (r:root) RETURN r.name as name LIMIT 1";
        let mut result = graph.execute(query(name_query)).await?;

        if let Ok(Some(row)) = result.next().await {
            let name: String = row.get("name").unwrap_or_else(|_| "我".to_string());
            Ok(name)
        } else {
            Ok("我".to_string())
        }
    }

    /// 辅助方法：静态更新名字（因为此时 Self 还没构建完）
    async fn update_root_name_static(graph: &Graph, name: &str) -> Result<()> {
        let query_str = "MATCH (r:root) SET r.name = $value";
        graph.run(query(query_str).param("value", name)).await?;
        info!("数据库根节点名字已同步为: {}", name);
        Ok(())
    }

    // 修改根节点name
    pub async fn update_root_name(&self, name: &str) -> Result<()> {
        let query_str = "MATCH (r:root) SET r.name = $value";
        self.graph
            .run(query(query_str).param("value", name))
            .await?;
        Ok(())
    }
}

impl Actor for AgentMemoryActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("AgentMemoryHActor 已启动");
    }
}

/// 1. 仅查询：获取当前相关的知识摘要
#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct QueryMemory {
    pub user_name: String,
}

impl Handler<QueryMemory> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: QueryMemory, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let agent_memory_prompt = self.agent_memory_prompt.clone();
        let user_name = msg.user_name;
        let enable_memory_query = self.enable_memory_query;

        Box::pin(
            async move {
                let result = fetch_all_entities(&this.graph).await.unwrap();
                let graph_str = format_graph_to_string(&result);
                // 如果智能记忆查询功能被禁用，直接返回空字符串
                if !enable_memory_query {
                    return Ok(graph_str);
                } else {
                    let prompt_query = agent_memory_prompt
                        .prompt_query
                        .replace("{content}", &graph_str);
                    let response_text = this
                        .open_aiproxy_actor
                        .send(CallOpenAI {
                            chat_completion_request_message: vec![
                                ChatCompletionRequestMessage::User(
                                    ChatCompletionRequestUserMessage {
                                        content: ChatCompletionRequestUserMessageContent::Text(
                                            prompt_query,
                                        ),
                                        name: Some(user_name.clone()),
                                    },
                                ),
                            ],
                            tools: None,
                            tool_choice: None,
                        })
                        .await
                        .map_err(ChatAgentError::from)??;
                    Ok(response_text)
                }
            }
            .into_actor(self),
        )
    }
}
/// 2. 仅更新：根据摘要和对话内容更新图数据库
#[derive(Message, Clone)]
#[rtype(result = "Result<(), ChatAgentError>")]
pub struct UpdateMemory {
    pub user_name: String,
    pub memory_content_short: Vec<ResultMessage>,
    pub message_content: MessageContent,
    pub current_knowledge_summary: String, // 传入查询到的摘要
}

impl Handler<UpdateMemory> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<(), ChatAgentError>>;

    fn handle(&mut self, msg: UpdateMemory, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let graph = self.graph.clone();
        let ai_name = self.ai_name.clone();
        let user_name = msg.user_name;
        let user_content = msg.message_content;
        let knowledge_summary = msg.current_knowledge_summary;
        let agent_memory_prompt = self.agent_memory_prompt.clone();
        let momory_short = msg.memory_content_short;

        Box::pin(
            async move {
                // 第一步：根据摘要决定增删改查
                let prompt_summary = agent_memory_prompt
                    .prompt_summary
                    .replace("{sender_name}", &user_name)
                    .replace("{content}", &user_content.to_string())
                    .replace("{momory_content_short}", &format!("{:#?}", momory_short))
                    .replace("{ai_name}", &ai_name)
                    .replace("{user_name}", &user_name)
                    .replace("{knowledge_summary}", &knowledge_summary);

                let op_response_text = this
                    .open_aiproxy_actor
                    .send(CallOpenAI {
                        chat_completion_request_message: vec![ChatCompletionRequestMessage::User(
                            ChatCompletionRequestUserMessage {
                                content: ChatCompletionRequestUserMessageContent::Text(
                                    prompt_summary,
                                ),
                                name: None,
                            },
                        )],
                        tools: None,
                        tool_choice: None,
                    })
                    .await
                    .map_err(ChatAgentError::from)??;

                let operations: Vec<Neo4jOperation> =
                    serde_json::from_str(&clean_json_string(&op_response_text)).map_err(|e| {
                        ChatAgentError::QueryError(format!("反序列化操作失败: {}", e))
                    })?;

                // 第二步：执行操作，处理约束冲突（肖悦已存在的情况）
                for operation in operations {
                    match operation.extract_operation_type(&graph).await {
                        Ok(_) => debug!("图操作成功"),
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.contains("ConstraintValidationFailed")
                                || msg.contains("already exists")
                            {
                                warn!("节点 {} 已存在，跳过创建操作", user_name);
                                continue;
                            } else {
                                return Err(ChatAgentError::QueryError(format!(
                                    "图数据库写入失败: {}",
                                    msg
                                )));
                            }
                        }
                    }
                }
                Ok(())
            }
            .into_actor(self),
        )
    }
}

// 获取自我名
#[derive(Message)]
#[rtype(result = "String")]
pub struct GetMyName {}
impl Handler<GetMyName> for AgentMemoryActor {
    type Result = String;

    fn handle(&mut self, _msg: GetMyName, _ctx: &mut Self::Context) -> Self::Result {
        self.ai_name.clone()
    }
}

impl Clone for AgentMemoryActor {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            ai_name: self.ai_name.clone(),
            agent_memory_prompt: self.agent_memory_prompt.clone(),
            enable_memory_query: self.enable_memory_query,
        }
    }
}
