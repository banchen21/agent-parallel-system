use actix::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use async_openai::types::chat::ChatCompletionRequestUserMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessageContent;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent,
};
use futures::future::join_all;
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
    // 状态：当前“本我”的识别名，初始默认为 Assistant
    pub ai_name: String,
    pub agent_memory_prompt: MemoryAgentConfig,
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
    ) -> Result<Self> {
        let graph = Graph::new(uri, user, pass).await?;

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
                    "CREATE (r:root {
                name: '我',
                created_at: datetime()
            })",
                ))
                .await?;
            info!("数据库中无根节点，初始化创建‘我’成功！");
        }

        let name_query = "MATCH (r:root) RETURN r.name as name";
        let mut name_res = graph.execute(query(name_query)).await?;
        let current_db_name: String = if let Ok(Some(row)) = name_res.next().await {
            row.get("name").unwrap_or_else(|_| "我".to_string())
        } else {
            "我".to_string()
        };

        let ai_name = current_db_name.clone();
        // 人格设定
        let raw_personality = Self::load_personality_file().await;
        let extract_prompt = format!(
            "
                # 任务
                从以下【人格设定文本】，提取出“名字”信息。
                要求仅输出 JSON：
                {{
                    \"name\": \"名字\"
                }}
                # 【人格设定文本】：\n{}",
            raw_personality
        );
        let json_res = match open_aiproxy_actor
            .send(CallOpenAI {
                chat_completion_request_message: vec![ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: ChatCompletionRequestUserMessageContent::Text(extract_prompt),
                        name: None,
                    },
                )],
            })
            .await
        {
            Ok(_s) => match _s {
                Ok(s) => s,
                Err(e) => {
                    error!("从人格设定中提取“本我”失败：{}", e);
                    return Err(anyhow!("从人格设定中提取“本我”失败：{}", e));
                }
            },
            Err(e) => {
                error!("从人格设定中提取“本我”失败：{}", e);
                return Err(anyhow!("从人格设定中提取“本我”失败：{}", e));
            }
        };
        let mut this = Self {
            graph: graph.clone(),
            open_aiproxy_actor: open_aiproxy_actor.clone(),
            ai_name,
            agent_memory_prompt: agent_memory_prompt.clone(),
        };
        let cleaned_json = clean_json_string(&json_res);
        let assistant_identity = serde_json::from_str::<AssistantIdentity>(&cleaned_json).unwrap();
        this.ai_name = assistant_identity.name;
        info!("从人格设定中提取“人格”：{}", this.ai_name);
        let _ = this.update_root_name(&this.ai_name).await;
        Ok(this)
    }

    // 修改根节点name
    pub async fn update_root_name(&self, name: &str) -> Result<()> {
        let query_str = "MATCH (r:root) SET r.name = $value";
        self.graph
            .run(query(query_str).param("value", name))
            .await?;
        Ok(())
    }

    // 查询节点的属性
    pub async fn query_node_property(&self, label: &str, name: &str) -> Result<String> {
        let query_str = format!(
            "
            MATCH (n:{label} {{name: $name}})
            UNWIND keys(n) AS key
            WITH key, n WHERE key <> 'created_at'
            RETURN key, toString(n[key]) as value
        "
        );

        let mut result = self
            .graph
            .execute(query(&query_str).param("name", name))
            .await
            .map_err(|e| anyhow!("查询实体属性失败: {}", e))?;

        let mut props = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let key: String = row.get("key").unwrap_or_default();
            let value: String = row.get("value").unwrap_or_default();
            props.push(format!("({}: {})", key, value));
        }

        if props.is_empty() {
            Ok(format!("- 未找到关于 [{}] 的任何信息", name))
        } else {
            Ok(format!(
                "- 关于 [{}] 的详细属性：\n{}",
                name,
                props.join("\n")
            ))
        }
    }

    /// 查询获取所有标签（labels）
    async fn get_all_labels(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // 执行 CALL db.labels() 并返回 label 列
        let mut result = self
            .graph
            .execute(query("CALL db.labels() YIELD label RETURN label"))
            .await?;

        let mut labels = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            // 从行中获取 "label" 字段，类型为 String
            let label: String = row.get("label")?;
            labels.push(label);
        }
        Ok(labels)
    }
}

impl Actor for AgentMemoryHActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("AgentMemoryHActor 已启动");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityContainer {
    // 这里的变量名必须和 JSON 中的 key 一模一样
    pub entitys: Vec<Entity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub label: String,
}

/// 1. 仅查询：获取当前相关的知识摘要
#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct QueryMemory {
    pub user_name: String,
    pub momory_content_short: Vec<ResultMessage>,
    pub message_content: MessageContent,
}

impl Handler<QueryMemory> for AgentMemoryHActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: QueryMemory, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let ai_name = self.ai_name.clone();
        let agent_memory_prompt = self.agent_memory_prompt.clone();
        let user_name = msg.user_name;
        let user_content = msg.message_content;
        let momory_short = msg.momory_content_short;

        Box::pin(
            async move {
                // 获取标签（建议处理错误，不要直接 unwrap）
                let labels = this
                    .get_all_labels()
                    .await
                    .map_err(|e| ChatAgentError::QueryError(e.to_string()))?;

                // 第一步：提取实体
                let prompt_query = agent_memory_prompt
                    .prompt_query
                    .replace("{ai_name}", &ai_name)
                    .replace("{labels}", &format!("{:?}", labels))
                    .replace("{user_name}", &user_name)
                    .replace("{knowledge_summary}", &format!("{:?}", momory_short));

                let response_text = this
                    .open_aiproxy_actor
                    .send(CallOpenAI {
                        chat_completion_request_message: vec![
                            ChatCompletionRequestMessage::System(
                                ChatCompletionRequestSystemMessage {
                                    content: ChatCompletionRequestSystemMessageContent::Text(
                                        prompt_query,
                                    ),
                                    name: None,
                                },
                            ),
                            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                                name: Some(user_name.clone()),
                                content: ChatCompletionRequestUserMessageContent::Text(
                                    user_content.to_string(),
                                ),
                            }),
                        ],
                    })
                    .await
                    .map_err(ChatAgentError::from)??;

                let entity_container: EntityContainer =
                    serde_json::from_str(&clean_json_string(&response_text))
                        .map_err(|e| ChatAgentError::QueryError(format!("解析实体失败: {}", e)))?;

                // --- 修复后的去重逻辑 ---
                use std::collections::HashSet;
                let mut seen_names = HashSet::new();
                let mut targets = Vec::new();

                // 强制添加核心实体
                seen_names.insert(ai_name.clone());
                targets.push((ai_name.clone(), "root".to_string()));

                if !seen_names.contains(&user_name) {
                    seen_names.insert(user_name.clone());
                    targets.push((user_name.clone(), "Entity".to_string()));
                }

                // 添加 AI 提取的实体
                for ent in entity_container.entitys {
                    if !seen_names.contains(&ent.name) {
                        seen_names.insert(ent.name.clone());
                        targets.push((ent.name, ent.label));
                    }
                }

                // 第二步：并发查询
                let entity_tasks = targets.into_iter().map(|(name, label)| {
                    let this = this.clone();
                    async move {
                        this.query_node_property(&label, &name).await.map_err(|e| {
                            ChatAgentError::QueryError(format!("查询实体[{}]失败: {}", name, e))
                        })
                    }
                });

                // collect 现在可以正确处理 Result<Vec<String>, ...> 了
                let entity_list: Vec<String> = join_all(entity_tasks)
                    .await
                    .into_iter()
                    .collect::<Result<Vec<String>, ChatAgentError>>()?;

                // 过滤掉“未找到”的内容并合并结果
                let final_summary = entity_list
                    .into_iter()
                    .filter(|s| !s.contains("未找到"))
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(final_summary)
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

                debug!("提示词预览： {}", prompt_summary);
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

                debug!("操作预览： {}", op_response_text);

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
        }
    }
}
