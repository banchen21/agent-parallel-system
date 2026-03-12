use actix::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use async_openai::types::chat::ChatCompletionRequestMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessageContent;
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::chat::actor_messages::ResultMessage;
use crate::chat::{
    chat_agent::PERSONALITY_PATH,
    model::MessageContent,
    openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor},
};
use crate::core::config::MemoryAgentConfig;
use crate::graph_memory::neo4j_model::fetch_all_entities;
use crate::graph_memory::neo4j_model::format_graph_to_string;
use crate::utils::json_util::clean_json_string;

/// 从人格设定中提取的“本我”结构
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AssistantIdentity {
    pub name: String, // 名字，如“齐悦”
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OperationType {
    /// 创建节点
    CreateNode,
    /// 对节点进行更新属性
    UpdateNodeProperty,
    /// 创建关系
    CreateRelationship,
    /// 更新关系
    UpdateRelationship,
    /// 更新关系的属性
    UpdateRelationshipProperty,
    /// 删除节点
    DeleteNode,
    /// 删除关系
    DeleteRelationship,
    /// 删除关系的属性
    DeleteRelationshipProperty,
    /// 删除节点属性
    DeleteNodeProperty,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Neo4jOperation {
    r#type: OperationType,
    label: Option<String>,
    properties: Option<HashMap<String, String>>,
}

impl Neo4jOperation {
    // 分类进行操作
    pub async fn extract_operation_type(&self, graph: &Graph) -> Result<()> {
        let properties = self
            .properties
            .as_ref()
            .ok_or_else(|| anyhow!("操作缺少 properties 字段"))?;
        let label = self
            .label
            .as_ref()
            .ok_or_else(|| anyhow!("操作缺少 label 字段"))?;

        match self.r#type {
            OperationType::CreateNode => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("CreateNode 中缺少 name 字段"))?;
                self.create_node(graph, label, name).await?;
            }
            OperationType::UpdateNodeProperty => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("UpdateNodeProperty 中缺少 name 字段"))?;

                // 优化：不再要求 AI 传嵌套 JSON 字符串
                // 直接遍历 properties HashMap，除了 "name" 以外的全当作属性更新
                for (key, value) in properties {
                    if key == "name" {
                        continue;
                    } // 跳过主键
                    self.update_node_property(graph, label, name, key, value)
                        .await?;
                }
            }
            OperationType::DeleteNodeProperty => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("DeleteNodeProperty 中缺少 name 字段"))?;
                let props_to_delete = properties
                    .get("properties")
                    .ok_or_else(|| anyhow!("DeleteNodeProperty 缺少 properties 字段"))?;

                // 优化：让 AI 用逗号分隔属性名，而不是 JSON 数组字符串
                for key in props_to_delete.split(',') {
                    let key = key.trim();
                    if !key.is_empty() {
                        self.delete_node_property(graph, label, name, key).await?;
                    }
                }
            }
            OperationType::CreateRelationship => {
                let to_label = properties
                    .get("to_label")
                    .ok_or_else(|| anyhow!("CreateRelationship 缺少 to_label"))?;
                let rel_type = properties
                    .get("type")
                    .ok_or_else(|| anyhow!("CreateRelationship 缺少 type"))?;
                let from = properties
                    .get("from")
                    .ok_or_else(|| anyhow!("CreateRelationship 缺少 from"))?;
                let end_name = properties
                    .get("end_name")
                    .ok_or_else(|| anyhow!("CreateRelationship 缺少 end_name"))?;

                self.create_relationship(graph, label, from, rel_type, to_label, end_name)
                    .await?;
            }
            OperationType::UpdateRelationship => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("UpdateRelationship 中缺少 name 字段"))?;
                let to_label = properties
                    .get("to_label")
                    .ok_or_else(|| anyhow!("UpdateRelationship 缺少 to_label"))?;
                let to_name = properties
                    .get("to_name")
                    .ok_or_else(|| anyhow!("UpdateRelationship 缺少 to_name"))?;
                let old_type = properties
                    .get("old_type")
                    .ok_or_else(|| anyhow!("UpdateRelationship 缺少 old_type"))?;
                let new_type = properties
                    .get("new_type")
                    .ok_or_else(|| anyhow!("UpdateRelationship 缺少 new_type"))?;

                self.update_relationship(graph, label, name, to_label, to_name, old_type, new_type)
                    .await?;
            }
            OperationType::UpdateRelationshipProperty => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("UpdateRelationshipProperty 中缺少 name 字段"))?;
                let to_label = properties
                    .get("to_label")
                    .ok_or_else(|| anyhow!("UpdateRelationshipProperty 缺少 to_label"))?;
                let to_name = properties
                    .get("to_name")
                    .ok_or_else(|| anyhow!("UpdateRelationshipProperty 缺少 to_name"))?;
                let rel_type = properties
                    .get("rel_type")
                    .ok_or_else(|| anyhow!("UpdateRelationshipProperty 缺少 rel_type"))?;
                let props_str = properties
                    .get("properties")
                    .ok_or_else(|| anyhow!("UpdateRelationshipProperty 缺少 properties"))?;
                let props: HashMap<String, String> = serde_json::from_str(props_str)
                    .map_err(|e| anyhow!("解析 properties 失败: {}", e))?;

                for (key, value) in props {
                    self.update_relationship_properties(
                        graph, label, name, to_label, to_name, rel_type, &key, &value,
                    )
                    .await?;
                }
            }
            OperationType::DeleteNode => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("DeleteNode 中缺少 name 字段"))?;
                self.delete_node(graph, label, name).await?;
            }
            OperationType::DeleteRelationship => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("DeleteRelationship 中缺少 name 字段"))?;
                let to_label = properties
                    .get("to_label")
                    .ok_or_else(|| anyhow!("DeleteRelationship 缺少 to_label"))?;
                let to_name = properties
                    .get("to_name")
                    .ok_or_else(|| anyhow!("DeleteRelationship 缺少 to_name"))?;
                let relation_type = properties
                    .get("relation_type")
                    .ok_or_else(|| anyhow!("DeleteRelationship 缺少 relation_type"))?;

                self.delete_relationship(graph, label, name, to_label, to_name, relation_type)
                    .await?;
            }
            OperationType::DeleteRelationshipProperty => {
                let name = properties
                    .get("name")
                    .ok_or_else(|| anyhow!("DeleteRelationshipProperty 中缺少 name 字段"))?;
                let to_label = properties
                    .get("to_label")
                    .ok_or_else(|| anyhow!("DeleteRelationshipProperty 缺少 to_label"))?;
                let to_name = properties
                    .get("to_name")
                    .ok_or_else(|| anyhow!("DeleteRelationshipProperty 缺少 to_name"))?;
                let rel_type = properties
                    .get("rel_type")
                    .ok_or_else(|| anyhow!("DeleteRelationshipProperty 缺少 rel_type"))?;
                let props_str = properties
                    .get("properties")
                    .ok_or_else(|| anyhow!("DeleteRelationshipProperty 缺少 properties"))?;
                let props: Vec<String> = serde_json::from_str(props_str)
                    .map_err(|e| anyhow!("解析 properties 失败: {}", e))?;

                for key in props {
                    self.delete_relationship_property(
                        graph, label, name, to_label, to_name, rel_type, &key,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    /// 创建实体
    pub async fn create_node(&self, graph: &Graph, entity: &str, name: &str) -> Result<()> {
        let query_str = format!("MERGE (n:{entity} {{name: $val}}) RETURN n");
        graph.run(query(&query_str).param("val", name)).await?;
        Ok(())
    }

    // 修改节点的属性
    pub async fn update_node_property(
        &self,
        graph: &Graph,
        label: &str, // 节点标签
        name: &str,  // 节点名称
        key: &str,   // 要更新的属性键
        value: &str, // 新的属性值
    ) -> Result<()> {
        let query_str = format!(
            "MATCH (n:{} {{name: $name}})
         SET n.{} = $value
         RETURN n",
            label, key
        );
        // 执行查询
        graph
            .run(query(&query_str).param("name", name).param("value", value))
            .await?;
        Ok(())
    }

    // 创建关系
    pub async fn create_relationship(
        &self,
        graph: &Graph,
        start_label: &str,
        from: &str,
        rel_type: &str,
        end_label: &str,
        end_name: &str,
    ) -> Result<bool> {
        // 1. 语法修正：ON CREATE 必须紧跟 MERGE
        // 2. 逻辑修正：利用 ON CREATE SET 一个临时标记，或者直接判断时间戳
        let query_str = format!(
            "MERGE (a:{start_label} {{name: $start_name}})
             MERGE (b:{end_label} {{name: $end_name}})
             WITH a, b
             // 先检查关系是否存在，用于返回 is_new 标记
             OPTIONAL MATCH (a)-[existing:{rel_type}]->(b)
             WITH a, b, (existing IS NULL) as is_new
             // 执行真正的 MERGE
             MERGE (a)-[r:{rel_type}]->(b)
             ON CREATE SET r.created_at = datetime()
             RETURN is_new",
        );

        let mut result = graph
            .execute(
                query(&query_str)
                    .param("start_name", from)
                    .param("end_name", end_name),
            )
            .await
            .map_err(|e| anyhow!("创建关系失败: {}", e))?;

        if let Ok(Some(row)) = result.next().await {
            let is_new: bool = row.get("is_new").unwrap_or(false);
            if is_new {
                debug!(
                    "创建了新关系: ({}:{}) -[{}]-> ({}:{})",
                    start_label, from, rel_type, end_label, end_name
                );
            } else {
                debug!(
                    "使用了已存在的关系: ({}:{}) -[{}]-> ({}:{})",
                    start_label, from, rel_type, end_label, end_name
                );
            }
            Ok(is_new)
        } else {
            Err(anyhow!("无法确定关系状态"))
        }
    }

    // 更新关系
    pub async fn update_relationship(
        &self,
        graph: &Graph,
        from_label: &str, // 起始节点标签
        from_name: &str,
        to_label: &str, // 结束节点标签
        to_name: &str,
        old_type: &str,
        new_type: &str,
    ) -> Result<()> {
        self.delete_relationship(graph, from_label, from_name, to_label, to_name, old_type)
            .await?;
        self.create_relationship(graph, from_label, from_name, new_type, to_label, to_name)
            .await?;
        Ok(())
    }

    // 更新关系的属性
    pub async fn update_relationship_properties(
        &self,
        graph: &Graph,
        from_label: &str,
        from_name: &str,
        to_label: &str,
        to_name: &str,
        rel_type: &str,
        key: &str,   // 属性名，例如 "weight"
        value: &str, // 属性值，例如 "100"
    ) -> Result<bool> {
        let query_str = format!(
            "MATCH (a:`{}`) -[r:`{}`]-> (b:`{}`) 
         WHERE a.name = $from_name AND b.name = $to_name
         SET r.`{}` = $value
         RETURN count(r) > 0 as success",
            from_label, rel_type, to_label, key
        );

        // 2. 执行查询
        let mut result = graph
            .execute(
                query(&query_str)
                    .param("from_name", from_name)
                    .param("to_name", to_name)
                    .param("value", value),
            )
            .await?;

        // 3. 解析结果
        // 如果 MATCH 到了关系，count(r) 会大于 0，返回 true
        if let Ok(Some(row)) = result.next().await {
            let is_success: bool = row.get("success").unwrap_or(false);
            if is_success {
                debug!("成功更新关系属性: [{}] 的 {} = {}", rel_type, key, value);
            } else {
                warn!(
                    "关系更新失败，未找到该关系: ({})-[{}]->({})",
                    from_name, rel_type, to_name
                );
            }
            Ok(is_success)
        } else {
            Ok(false)
        }
    }

    // 删除节点及其关系
    pub async fn delete_node(&self, graph: &Graph, label: &str, name: &str) -> Result<()> {
        let query_str = format!("MATCH (n:{label} {{name: $name}}) DETACH DELETE n");
        graph.run(query(&query_str).param("name", name)).await?;
        debug!("已彻底删除节点 {} 及其所有关联关系", name);
        Ok(())
    }

    // 删除特定关系
    pub async fn delete_relationship(
        &self,
        graph: &Graph,
        label: &str,
        name: &str,
        to_label: &str,
        to_name: &str,
        relation_type: &str,
    ) -> Result<()> {
        // 使用反引号 `` 包裹动态拼接的标识符，防止特殊字符导致语法错误
        let query_str = format!(
            "MATCH (a:`{label}` {{name: $name}})-[r:`{relation_type}`]->(b:`{to_label}` {{name: $to_name}}) DELETE r"
        );

        graph
            .run(
                query(&query_str)
                    .param("name", name)
                    .param("to_name", to_name),
            )
            .await?;

        debug!(
            "尝试删除关系: ({})-[{}]->({})",
            name, relation_type, to_name
        );
        Ok(())
    }

    /// 删除关系的属性
    pub async fn delete_relationship_property(
        &self,
        graph: &Graph,
        label: &str,
        name: &str,
        to_label: &str,
        to_name: &str,
        relation_type: &str,
        key: &str,
    ) -> Result<()> {
        let query_str = format!(
            "MATCH (a:`{label}` {{name: $name}})-[r:`{relation_type}`]->(b:`{to_label}` {{name: $to_name}}) REMOVE r.`{key}`"
        );
        graph
            .run(
                query(&query_str)
                    .param("name", name)
                    .param("to_name", to_name),
            )
            .await?;
        debug!("已删除关系 {} 的属性 {}", relation_type, key);
        Ok(())
    }

    /// 删除节点属性
    pub async fn delete_node_property(
        &self,
        graph: &Graph,
        label: &str,
        name: &str,
        key: &str,
    ) -> Result<()> {
        let query_str = format!("MATCH (n:{label} {{name: $name}}) REMOVE n.`{key}`");
        graph.run(query(&query_str).param("name", name)).await?;
        debug!("已删除节点 {} 的属性 {}", name, key);
        Ok(())
    }
}

pub struct AgentMemoryHActor {
    pub graph: Graph,
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
    // 状态：当前"本我"的识别名，初始默认为 Assistant
    pub ai_name: String,
    pub agent_memory_prompt: MemoryAgentConfig,
    pub enable_memory_query: bool,
    pub max_query_depth: u32,
}
impl AgentMemoryHActor {
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
        max_query_depth: u32,
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
            max_query_depth,
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

impl Actor for AgentMemoryHActor {
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

impl Handler<QueryMemory> for AgentMemoryHActor {
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

impl Handler<UpdateMemory> for AgentMemoryHActor {
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
impl Handler<GetMyName> for AgentMemoryHActor {
    type Result = String;

    fn handle(&mut self, _msg: GetMyName, _ctx: &mut Self::Context) -> Self::Result {
        self.ai_name.clone()
    }
}

impl Clone for AgentMemoryHActor {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            ai_name: self.ai_name.clone(),
            agent_memory_prompt: self.agent_memory_prompt.clone(),
            enable_memory_query: self.enable_memory_query,
            max_query_depth: self.max_query_depth,
        }
    }
}
