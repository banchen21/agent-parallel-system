use actix::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use async_openai::types::chat::ChatCompletionRequestMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessageContent;
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::channel::actor_messages::ResultMessage;
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

#[derive(Debug, Clone, Serialize)]
pub struct MemoryNodeDto {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub metadata: HashMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryRelationshipDto {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
    pub metadata: HashMap<String, String>,
    pub created_at: String,
}

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
                provider: None,
                model: None,
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
                            provider: None,
                            model: None,
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
                        provider: None,
                        model: None,
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

#[derive(Message)]
#[rtype(result = "Result<Vec<MemoryNodeDto>, ChatAgentError>")]
pub struct ListMemoryNodes {
    pub query: Option<String>,
}

#[derive(Message)]
#[rtype(result = "Result<MemoryNodeDto, ChatAgentError>")]
pub struct CreateMemoryNode {
    pub name: String,
    pub description: String,
    pub node_type: String,
}

#[derive(Message)]
#[rtype(result = "Result<MemoryNodeDto, ChatAgentError>")]
pub struct UpdateMemoryNode {
    pub node_id: i64,
    pub name: String,
    pub description: String,
    pub node_type: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), ChatAgentError>")]
pub struct DeleteMemoryNode {
    pub node_id: i64,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<MemoryRelationshipDto>, ChatAgentError>")]
pub struct ListNodeRelationships {
    pub node_id: i64,
}

#[derive(Message)]
#[rtype(result = "Result<MemoryRelationshipDto, ChatAgentError>")]
pub struct CreateMemoryRelationship {
    pub source_id: i64,
    pub target_id: i64,
    pub relationship_type: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), ChatAgentError>")]
pub struct DeleteMemoryRelationship {
    pub relationship_id: i64,
}

fn sanitize_relationship_type(raw: &str) -> String {
    let mut s = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "RELATED_TO".to_string()
    } else {
        s
    }
}

impl Handler<ListMemoryNodes> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<Vec<MemoryNodeDto>, ChatAgentError>>;

    fn handle(&mut self, msg: ListMemoryNodes, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                let mut nodes: Vec<MemoryNodeDto> = Vec::new();
                if let Some(q) = msg.query.and_then(|s| {
                    let t = s.trim().to_string();
                    if t.is_empty() { None } else { Some(t) }
                }) {
                    let mut result = graph
                        .execute(
                            query(
                                "MATCH (n) WHERE toLower(coalesce(n.name, '')) CONTAINS toLower($q) OR toLower(coalesce(n.description, '')) CONTAINS toLower($q) RETURN id(n) AS id, labels(n) AS labels, coalesce(n.name, '') AS name, coalesce(n.description, '') AS description, coalesce(n.type, '') AS type, coalesce(toString(n.created_at), '') AS created_at, coalesce(toString(n.updated_at), '') AS updated_at ORDER BY id DESC LIMIT 300",
                            )
                            .param("q", q),
                        )
                        .await?;

                    while let Ok(Some(row)) = result.next().await {
                        let id: i64 = row.get("id").unwrap_or(0);
                        let labels: Vec<String> = row.get("labels").unwrap_or_default();
                        let name: String = row.get("name").unwrap_or_default();
                        let description: String = row.get("description").unwrap_or_default();
                        let raw_type: String = row.get("type").unwrap_or_default();
                        let node_type = if raw_type.trim().is_empty() {
                            labels
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "unknown".to_string())
                        } else {
                            raw_type
                        };
                        let created_at: String = row.get("created_at").unwrap_or_default();
                        let updated_at: String = row.get("updated_at").unwrap_or_default();
                        nodes.push(MemoryNodeDto {
                            id: id.to_string(),
                            name,
                            description,
                            node_type,
                            metadata: HashMap::new(),
                            created_at,
                            updated_at,
                        });
                    }
                } else {
                    let mut result = graph
                        .execute(query("MATCH (n) RETURN id(n) AS id, labels(n) AS labels, coalesce(n.name, '') AS name, coalesce(n.description, '') AS description, coalesce(n.type, '') AS type, coalesce(toString(n.created_at), '') AS created_at, coalesce(toString(n.updated_at), '') AS updated_at ORDER BY id DESC LIMIT 300"))
                        .await?;

                    while let Ok(Some(row)) = result.next().await {
                        let id: i64 = row.get("id").unwrap_or(0);
                        let labels: Vec<String> = row.get("labels").unwrap_or_default();
                        let name: String = row.get("name").unwrap_or_default();
                        let description: String = row.get("description").unwrap_or_default();
                        let raw_type: String = row.get("type").unwrap_or_default();
                        let node_type = if raw_type.trim().is_empty() {
                            labels
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "unknown".to_string())
                        } else {
                            raw_type
                        };
                        let created_at: String = row.get("created_at").unwrap_or_default();
                        let updated_at: String = row.get("updated_at").unwrap_or_default();
                        nodes.push(MemoryNodeDto {
                            id: id.to_string(),
                            name,
                            description,
                            node_type,
                            metadata: HashMap::new(),
                            created_at,
                            updated_at,
                        });
                    }
                }

                Ok(nodes)
            }
            .into_actor(self),
        )
    }
}

impl Handler<CreateMemoryNode> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<MemoryNodeDto, ChatAgentError>>;

    fn handle(&mut self, msg: CreateMemoryNode, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                let mut result = graph
                    .execute(
                        query("CREATE (n:Memory {name: $name, description: $description, type: $type, created_at: datetime(), updated_at: datetime()}) RETURN id(n) AS id, coalesce(n.name, '') AS name, coalesce(n.description, '') AS description, coalesce(n.type, 'unknown') AS type, toString(n.created_at) AS created_at, toString(n.updated_at) AS updated_at")
                            .param("name", msg.name)
                            .param("description", msg.description)
                            .param("type", msg.node_type),
                    )
                    .await?;

                if let Ok(Some(row)) = result.next().await {
                    let id: i64 = row.get("id").unwrap_or(0);
                    let name: String = row.get("name").unwrap_or_default();
                    let description: String = row.get("description").unwrap_or_default();
                    let node_type: String = row.get("type").unwrap_or_else(|_| "unknown".to_string());
                    let created_at: String = row.get("created_at").unwrap_or_default();
                    let updated_at: String = row.get("updated_at").unwrap_or_default();
                    Ok(MemoryNodeDto {
                        id: id.to_string(),
                        name,
                        description,
                        node_type,
                        metadata: HashMap::new(),
                        created_at,
                        updated_at,
                    })
                } else {
                    Err(ChatAgentError::QueryError("创建节点失败".to_string()))
                }
            }
            .into_actor(self),
        )
    }
}

impl Handler<UpdateMemoryNode> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<MemoryNodeDto, ChatAgentError>>;

    fn handle(&mut self, msg: UpdateMemoryNode, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                let mut result = graph
                    .execute(
                        query("MATCH (n) WHERE id(n) = $id SET n.name = $name, n.description = $description, n.type = $type, n.updated_at = datetime() RETURN id(n) AS id, coalesce(n.name, '') AS name, coalesce(n.description, '') AS description, coalesce(n.type, 'unknown') AS type, coalesce(toString(n.created_at), '') AS created_at, toString(n.updated_at) AS updated_at")
                            .param("id", msg.node_id)
                            .param("name", msg.name)
                            .param("description", msg.description)
                            .param("type", msg.node_type),
                    )
                    .await?;

                if let Ok(Some(row)) = result.next().await {
                    let id: i64 = row.get("id").unwrap_or(0);
                    let name: String = row.get("name").unwrap_or_default();
                    let description: String = row.get("description").unwrap_or_default();
                    let node_type: String = row.get("type").unwrap_or_else(|_| "unknown".to_string());
                    let created_at: String = row.get("created_at").unwrap_or_default();
                    let updated_at: String = row.get("updated_at").unwrap_or_default();
                    Ok(MemoryNodeDto {
                        id: id.to_string(),
                        name,
                        description,
                        node_type,
                        metadata: HashMap::new(),
                        created_at,
                        updated_at,
                    })
                } else {
                    Err(ChatAgentError::QueryError("节点不存在".to_string()))
                }
            }
            .into_actor(self),
        )
    }
}

impl Handler<DeleteMemoryNode> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<(), ChatAgentError>>;

    fn handle(&mut self, msg: DeleteMemoryNode, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                graph
                    .run(query("MATCH (n) WHERE id(n) = $id DETACH DELETE n").param("id", msg.node_id))
                    .await?;
                Ok(())
            }
            .into_actor(self),
        )
    }
}

impl Handler<ListNodeRelationships> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<Vec<MemoryRelationshipDto>, ChatAgentError>>;

    fn handle(&mut self, msg: ListNodeRelationships, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                let mut result = graph
                    .execute(
                        query("MATCH (a)-[r]->(b) WHERE id(a) = $id RETURN id(r) AS id, id(a) AS source_id, id(b) AS target_id, type(r) AS relationship_type, coalesce(toString(r.created_at), '') AS created_at ORDER BY id DESC")
                            .param("id", msg.node_id),
                    )
                    .await?;

                let mut rels = Vec::new();
                while let Ok(Some(row)) = result.next().await {
                    let id: i64 = row.get("id").unwrap_or(0);
                    let source_id: i64 = row.get("source_id").unwrap_or(0);
                    let target_id: i64 = row.get("target_id").unwrap_or(0);
                    let relationship_type: String = row.get("relationship_type").unwrap_or_else(|_| "RELATED_TO".to_string());
                    let created_at: String = row.get("created_at").unwrap_or_default();
                    rels.push(MemoryRelationshipDto {
                        id: id.to_string(),
                        source_id: source_id.to_string(),
                        target_id: target_id.to_string(),
                        relationship_type,
                        metadata: HashMap::new(),
                        created_at,
                    });
                }
                Ok(rels)
            }
            .into_actor(self),
        )
    }
}

impl Handler<CreateMemoryRelationship> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<MemoryRelationshipDto, ChatAgentError>>;

    fn handle(
        &mut self,
        msg: CreateMemoryRelationship,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                let rel_type = sanitize_relationship_type(&msg.relationship_type);
                let cypher = format!(
                    "MATCH (a), (b) WHERE id(a) = $source_id AND id(b) = $target_id CREATE (a)-[r:{} {{created_at: datetime()}}]->(b) RETURN id(r) AS id, id(a) AS source_id, id(b) AS target_id, type(r) AS relationship_type, toString(r.created_at) AS created_at",
                    rel_type
                );

                let mut result = graph
                    .execute(
                        query(&cypher)
                            .param("source_id", msg.source_id)
                            .param("target_id", msg.target_id),
                    )
                    .await?;

                if let Ok(Some(row)) = result.next().await {
                    let id: i64 = row.get("id").unwrap_or(0);
                    let source_id: i64 = row.get("source_id").unwrap_or(0);
                    let target_id: i64 = row.get("target_id").unwrap_or(0);
                    let relationship_type: String = row.get("relationship_type").unwrap_or_else(|_| "RELATED_TO".to_string());
                    let created_at: String = row.get("created_at").unwrap_or_default();
                    Ok(MemoryRelationshipDto {
                        id: id.to_string(),
                        source_id: source_id.to_string(),
                        target_id: target_id.to_string(),
                        relationship_type,
                        metadata: HashMap::new(),
                        created_at,
                    })
                } else {
                    Err(ChatAgentError::QueryError("创建关系失败".to_string()))
                }
            }
            .into_actor(self),
        )
    }
}

impl Handler<DeleteMemoryRelationship> for AgentMemoryActor {
    type Result = ResponseActFuture<Self, Result<(), ChatAgentError>>;

    fn handle(
        &mut self,
        msg: DeleteMemoryRelationship,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let graph = self.graph.clone();
        Box::pin(
            async move {
                graph
                    .run(
                        query("MATCH ()-[r]->() WHERE id(r) = $id DELETE r")
                            .param("id", msg.relationship_id),
                    )
                    .await?;
                Ok(())
            }
            .into_actor(self),
        )
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
