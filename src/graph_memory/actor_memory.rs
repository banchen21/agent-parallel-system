use actix::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use async_openai::types::chat::ChatCompletionRequestUserMessage;
use async_openai::types::chat::ChatCompletionRequestUserMessageContent;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent,
};
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::chat::actor_messages::ResultMessage;
use crate::core::config::MemoryAgentConfig;
// 引用你的工程中已有的模块
use crate::{
    chat::{
        chat_agent::PERSONALITY_PATH,
        model::MessageContent,
        openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor},
    },
    utils::json_util::clean_json_string,
};

/// 从人格设定中提取的“本我”结构
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AssistantIdentity {
    pub name: String, // 名字，如“齐悦”
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

        let check_query = "MATCH (root:根节点) RETURN count(root) as exists";
        let mut result = graph.execute(query(check_query)).await?;
        let count: i64 = if let Ok(Some(row)) = result.next().await {
            row.get("exists").unwrap_or(0)
        } else {
            0
        };

        if count == 0 {
            graph
                .run(query(
                    "CREATE (root:根节点 {
                name: '我',
                created_at: datetime()
            })",
                ))
                .await?;
            info!("数据库中无根节点，初始化创建‘我’成功！");
        } else {
            info!("数据库中已存在根节点，跳过初始化创建。");
        }

        let name_query = "MATCH (root:根节点) RETURN root.name as name";
        let mut name_res = graph.execute(query(name_query)).await?;
        let current_db_name: String = if let Ok(Some(row)) = name_res.next().await {
            row.get("name").unwrap_or_else(|_| "我".to_string())
        } else {
            "我".to_string()
        };

        let mut ai_name = current_db_name.clone();
        let this = Self {
            graph: graph.clone(),
            open_aiproxy_actor: open_aiproxy_actor.clone(),
            ai_name: ai_name.clone(),
            agent_memory_prompt: agent_memory_prompt.clone(),
        };
        // 3. 核心判断逻辑：如果是“我”，则请求 AI 提取身份
        //TODO: 缺乏兼容性
        if current_db_name == "我" {
            let raw_personality = Self::load_personality_file().await;
            let extract_prompt = format!(
                "你是一个身份分析专家。请阅读以下【人格设定文本】，提取出该角色的‘本我’信息。
                要求仅输出 JSON：
                {{
                    \"name\": \"名字\"
                }}
                【人格设定文本】：\n{}",
                raw_personality
            );
            info!("正在后台通过 AI 代理从人格设定中提取“本我”...");
            let json_res = match open_aiproxy_actor
                .send(CallOpenAI {
                    chat_completion_request_message: vec![ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                extract_prompt,
                            ),
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
            let cleaned_json = clean_json_string(&json_res);
            if let Ok(assistant_identity) = serde_json::from_str::<AssistantIdentity>(&cleaned_json)
            {
                ai_name = assistant_identity.name.clone();
            }
            match this.update_root_name(&ai_name).await {
                Ok(()) => {
                    info!("从人格设定中提取“本我”成功：{}", ai_name);
                }
                Err(e) => {
                    error!("从人格设定中提取“本我”失败：{}", e);
                    return Err(anyhow!("从人格设定中提取“本我”失败：{}", e));
                }
            }
        } else {
            info!("当前根节点名已是“{}”，跳过 AI 提取步骤。", current_db_name);
        }
        Ok(this)
    }

    // 修改根节点name
    pub async fn update_root_name(&self, name: &str) -> Result<()> {
        let query_str = "MATCH (root:根节点) SET root.name = $value";
        self.graph
            .run(query(query_str).param("value", name))
            .await?;
        Ok(())
    }

    /// 1. 创建实体
    pub async fn create_node(&self, name: &str) -> Result<()> {
        let query_str = "CREATE (n:Entity) SET n.name = $val";
        self.graph.run(query(query_str).param("val", name)).await?;
        Ok(())
    }
    /// 2. 修改实体属性 (例如：更新 :记忆点 的权重或内容)
    pub async fn update_node_property(&self, name: &str, key: &str, value: &str) -> Result<()> {
        let query_str = format!("MATCH (n:Entity {{name: $name}}) SET n.`{}` = $value", key);

        self.graph
            .run(query(&query_str).param("name", name).param("value", value))
            .await?;

        Ok(())
    }

    // 创建根节点与实体之间的关系
    pub async fn create_root_node_relationship(
        &self,
        rel_type: &str,
        entity_name: &str,
    ) -> Result<()> {
        let query_str = format!(
            "MATCH (root:根节点) 
             MERGE (e:Entity {{name: $entity_name}}) 
             MERGE (root)-[r:{}]->(e) 
             SET r.created_at = datetime()",
            rel_type
        );

        self.graph
            .run(query(&query_str).param("entity_name", entity_name))
            .await?;

        debug!(
            "成功创建自我记忆链: (本我) -[{}]-> ({})",
            rel_type, entity_name
        );
        Ok(())
    }

    // 3. 创建关系
    pub async fn create_relationship(
        &self,
        start_name: &str,
        rel_type: &str,
        end_name: &str,
    ) -> Result<()> {
        let query_str = format!(
            "MERGE (a:Entity {{name: $start}}) 
             MERGE (b:Entity {{name: $end}}) 
             MERGE (a)-[r:{}]->(b) 
             SET r.created_at = datetime()",
            rel_type
        );

        self.graph
            .run(
                query(&query_str)
                    .param("start", start_name)
                    .param("end", end_name),
            )
            .await?;

        debug!(
            "成功创建关系: ({}) -[{}]-> ({})",
            start_name, rel_type, end_name
        );
        Ok(())
    }

    /// 删除节点及其所有关联的关系 (使用 DETACH 自动清理所有悬空边)
    pub async fn delete_node(&self, name: &str) -> Result<()> {
        let query_str = "MATCH (n:Entity {name: $name}) DETACH DELETE n";
        self.graph.run(query(query_str).param("name", name)).await?;
        debug!("已彻底删除节点 {} 及其所有关联关系", name);
        Ok(())
    }

    /// 删除两个节点之间的特定关系
    pub async fn delete_relationship(
        &self,
        start_name: &str,
        rel_type: &str,
        end_name: &str,
    ) -> Result<()> {
        // 同样使用白名单校验 rel_type 的安全性
        let allowed_rels = ["拥有记忆", "RELATED_TO", "喜欢", "居住在", "工作于"];
        if !allowed_rels.contains(&rel_type) {
            return Err(anyhow::anyhow!("不支持的关系类型: {}", rel_type));
        }

        let query_str = format!(
            "MATCH (a:Entity {{name: $start}})-[r:{}]->(b:Entity {{name: $end}}) DELETE r",
            rel_type
        );

        self.graph
            .run(
                query(&query_str)
                    .param("start", start_name)
                    .param("end", end_name),
            )
            .await?;

        debug!(
            "已删除关系: ({}) -[{}]-> ({})",
            start_name, rel_type, end_name
        );
        Ok(())
    }

    /// 查询（根节点）有关系的节点
    pub async fn query_me_nodes(&self) -> Result<String> {
        // MATCH 说明：匹配唯一的 :根节点，找出它所有的 1 跳出向关系
        let query_str = "
            MATCH (root:根节点)-[r]->(neighbor)
            RETURN root.name as me_name, type(r) as rel, neighbor.name as target_name
            LIMIT 50
        ";

        let mut result = self.graph.execute(query(query_str)).await?;
        let mut lines = Vec::new();

        while let Ok(Some(row)) = result.next().await {
            let me: String = row.get("name").unwrap_or_else(|_| "我".into());
            let rel: String = row.get("rel").unwrap_or_else(|_| "相关".into());
            let target: String = row.get("target_name").unwrap_or_else(|_| "未知".into());

            lines.push(format!("{} -> ({}) -> ({})", me, rel, target));
        }

        if lines.is_empty() {
            Ok(self.ai_name.clone())
        } else {
            Ok(lines.join("\n"))
        }
    }

    /// 查询和“用户”有关系的节点
    pub async fn query_user_nodes(&self, user_name: &str) -> Result<String> {
        // MATCH 说明：匹配标签为 :Entity 且名字为 user_name 的节点
        let query_str = "
            MATCH (u:Entity {name: $user_name})-[r]->(neighbor)
            RETURN u.name as user, type(r) as rel, neighbor.name as target
            ORDER BY neighbor.created_at DESC
            LIMIT 50
        ";

        let mut result = self
            .graph
            .execute(query(query_str).param("user_name", user_name))
            .await?;

        let mut lines = Vec::new();

        while let Ok(Some(row)) = result.next().await {
            let user: String = row.get("user").unwrap_or_else(|_| user_name.into());
            let rel: String = row.get("rel").unwrap_or_else(|_| "记录".into());
            let target: String = row.get("target").unwrap_or_else(|_| "内容".into());

            lines.push(format!("{} -> ({}) -> ({})", user, rel, target));
        }

        if lines.is_empty() {
            Ok(format!("{}", user_name))
        } else {
            Ok(lines.join("\n"))
        }
    }

    /// 查询根节点的属性
    pub async fn query_root_node_property(&self) -> Result<String> {
        let query_str = "
            MATCH (n:根节点)
            UNWIND keys(n) AS key
            WITH key, n WHERE key <> 'created_at'
            RETURN key, toString(n[key]) as value
        ";

        let mut result = self
            .graph
            .execute(query(query_str))
            .await
            .map_err(|e| anyhow!("读取根节点属性失败: {}", e))?;

        let mut props = Vec::new();

        // 遍历查询结果
        while let Ok(Some(row)) = result.next().await {
            let key: String = row.get("key").unwrap_or_default();
            let value: String = row.get("value").unwrap_or_default();
            // 格式化为：(属性名: 属性值)
            props.push(format!("({}: {})", key, value));
        }

        if props.is_empty() {
            Ok("目前没有任何关于自己的任何信息。".to_string())
        } else {
            // 将所有属性拼接成一段话
            Ok(format!("[{}]的属性：{:?}", self.ai_name, props))
        }
    }

    /// 2. 查询普通“实体节点”的所有属性
    /// 适用于查询用户、地点、物品等实体的详细信息
    pub async fn query_user_node_property(&self, name: &str) -> Result<String> {
        let query_str = "
            MATCH (n:Entity {name: $name})
            UNWIND keys(n) AS key
            WITH key, n WHERE key <> 'created_at'
            RETURN key, toString(n[key]) as value
        ";

        let mut result = self
            .graph
            .execute(query(query_str).param("name", name))
            .await
            .map_err(|e| anyhow!("查询实体属性失败: {}", e))?;

        let mut props = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let key: String = row.get("key").unwrap_or_default();
            let value: String = row.get("value").unwrap_or_default();
            props.push(format!("({}: {})", key, value));
        }

        if props.is_empty() {
            Ok(format!("未找到关于 [{}] 的任何信息", name))
        } else {
            Ok(format!(
                "实体 [{}] 的详细属性：\n{}",
                name,
                props.join("\n")
            ))
        }
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
    pub entitys: Vec<String>,
}

/// 外部请求消息：仅传入用户名和当前对话内容
#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct RequestMemory {
    pub user_name: String,
    pub momory_content_short: Vec<ResultMessage>,
    pub message_content: MessageContent,
}
// --- 核心业务 Handler ---
impl Handler<RequestMemory> for AgentMemoryHActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: RequestMemory, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let user_name = msg.user_name.clone();
        let user_content = msg.message_content.clone();
        let ai_name = self.ai_name.clone();
        let agent_memory_prompt = self.agent_memory_prompt.clone();
        let momory_content_short = msg.momory_content_short.clone();

        Box::pin(
            async move {
                // 查询所有实体
                // 1.构建提示词
                let prompt_query = agent_memory_prompt.prompt_query;
                let new_prompt_query = prompt_query.replace("{user_name}", &user_name).replace(
                    "{knowledge_summary}",
                    format!("{:?}", momory_content_short).as_str(),
                );
                // 获取查询所有实体响应
                let response_text = this
                    .open_aiproxy_actor
                    .send(CallOpenAI {
                        chat_completion_request_message: vec![
                            ChatCompletionRequestMessage::System(
                                ChatCompletionRequestSystemMessage {
                                    content: ChatCompletionRequestSystemMessageContent::Text(
                                        new_prompt_query,
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

                let entity_container: EntityContainer =match serde_json::from_str(&clean_json_string(&response_text)) {
                    Ok(res) => res,
                    Err(e) => 
                    {
                        error!("反序列化失败：{}", e);
                        return Err(ChatAgentError::QueryError(e.to_string()));
                    },
                };

                // 所有需要查询的实体
                let mut entity_list = vec![];
                for entity in entity_container.entitys {
                    if entity == ai_name {
                        // 和自己有关系的实体
                        match this.query_me_nodes().await {
                            Ok(response) => {
                                entity_list.push(response);
                            }
                            Err(_e) => {
                                error!("查询根节点失败：{}", _e);
                                return Err(ChatAgentError::QueryError(_e.to_string()));
                            }
                        }
                    } else {
                        // 和用户有关系的实体
                        match this.query_user_nodes(&entity).await {
                            Ok(response) => {
                                entity_list.push(response);
                            }
                            Err(_e) => {
                                error!("查询失败：{}", _e);
                                return Err(ChatAgentError::QueryError(_e.to_string()));
                            }
                        }
                    }
                }

                // 2.实体分析
                let mut user_property_list = vec![];
                for entity in entity_list {
                    if entity == ai_name {
                        let property = this.query_root_node_property().await.map_err(|e| {
                            error!("查询失败：{}", e);
                            ChatAgentError::QueryError(e.to_string())
                        })?;
                        user_property_list.push(property);
                    } else {
                        let property =
                            this.query_user_node_property(&entity).await.map_err(|e| {
                                error!("查询失败：{}", e);
                                ChatAgentError::QueryError(e.to_string())
                            })?;
                        user_property_list.push(property);
                    }
                }
                // 正确处理节点与关系
                // 1.构建提示词
                let prompt_summary = agent_memory_prompt.prompt_summary;
                let new_prompt_summary = prompt_summary
                    .replace("{ai_name}", &ai_name)
                    .replace("{user_name}", &user_name)
                    .replace(
                        "{knowledge_summary}",
                        format!("{:?}", user_property_list).as_str(),
                    );

                // 进行：创建、删除、修改节点与关系的响应
                let insert_delete_update_response_text = this
                    .open_aiproxy_actor
                    .send(CallOpenAI {
                        chat_completion_request_message: vec![
                            ChatCompletionRequestMessage::System(
                                ChatCompletionRequestSystemMessage {
                                    content: ChatCompletionRequestSystemMessageContent::Text(
                                        new_prompt_summary,
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
                debug!("4、预览操作响应：{}", insert_delete_update_response_text);
                //TODO: 实现操作
                Ok(format!(" {}\n", "knowledge_summary"))
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
