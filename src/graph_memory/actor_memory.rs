use actix::prelude::*;
use anyhow::Result;
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// 引用你的工程中已有的模块
use crate::{
    chat::{
        chat_agent::ChatAgentConfig,
        openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor},
    },
    utils::json_util::clean_json_string,
};

// --- 数据结构定义 ---

/// 从人格设定中提取的“本我”结构
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AssistantIdentity {
    pub name: String,        // 名字，如“齐悦”
    pub role: String,        // 角色，如“赛博女儿”
    pub traits: Vec<String>, // 性格标签
    pub core_goal: String,   // 核心目标
}

/// 外部请求消息：仅传入用户名和当前对话内容
#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct RequestMemory {
    pub user_name: String,
    pub message_content: String,
}

// --- Actor 定义 ---

pub struct AgentMemoryHActor {
    pub graph: Graph,
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
    pub config: ChatAgentConfig,
    // 状态：当前“本我”的识别名，初始默认为 Assistant
    pub ai_name: String,
    pub agent_memory_prompt_template: String,
}

impl AgentMemoryHActor {
    /// 初始化并启动 Actor
    pub async fn new(
        uri: &str,
        user: &str,
        pass: &str,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        agent_memory_prompt_template: String,
    ) -> Result<Self> {
        let graph = Graph::new(uri, user, pass)
            .await
            .map_err(|e| anyhow::anyhow!("Neo4j 连接失败: {}", e))?;

        let mut actor = Self {
            graph,
            open_aiproxy_actor,
            config: ChatAgentConfig::default(),
            ai_name: "Assistant".to_string(),
            agent_memory_prompt_template,
        };

        // 1. 基础连接校验
        actor.graph.run(query("RETURN 1")).await?;

        // 2. 核心步骤：从人格设定文件中自动“提取本我”并同步到 Neo4j
        actor.init_self_identity().await?;

        Ok(actor)
    }

    /// 【本我提取逻辑】读取 MD 文件并让 AI 解析
    async fn init_self_identity(&mut self) -> Result<()> {
        let raw_personality = self.load_personality_file().await;

        let extract_prompt = format!(
            "你是一个身份分析专家。请阅读以下【人格设定文本】，提取出该角色的‘本我’信息。
            要求仅输出 JSON：
            {{
                \"name\": \"名字\",
                \"role\": \"角色定位\",
                \"traits\": [\"标签1\", \"标签2\"],
                \"core_goal\": \"核心目标\"
            }}
            文本内容：\n{}",
            raw_personality
        );

        info!("正在通过 AI 代理从人格设定中提取“本我”特征...");

        if let Ok(Ok(json_res)) = self
            .open_aiproxy_actor
            .send(CallOpenAI {
                prompt: extract_prompt,
            })
            .await
        {
            let cleaned_json = clean_json_string(&json_res);

            if let Ok(id) = serde_json::from_str::<AssistantIdentity>(cleaned_json) {
                self.ai_name = id.name.clone();
                info!("✨ 成功识别本我身份: {} ({})", self.ai_name, id.role);

                // 同步到 Neo4j
                let sync_q = "
                    MERGE (a:Assistant {name: $name})
                    SET a.role = $role, 
                        a.traits = $traits, 
                        a.core_goal = $goal, 
                        a.is_self = true,
                        a.last_sync = timestamp()
                ";
                self.graph
                    .run(
                        query(sync_q)
                            .param("name", id.name)
                            .param("role", id.role)
                            .param("traits", id.traits)
                            .param("goal", id.core_goal),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn load_personality_file(&self) -> String {
        let path = self.config.personality_path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::read_to_string(&path).unwrap_or_else(|_| "你是一个智能助手".to_string())
        })
        .await
        .unwrap_or_default()
    }

    /// 【查】获取用户的结构化记忆摘要
    async fn get_user_facts(&self, user_name: &str) -> Result<String> {
        let search_q = "
            MATCH (u {name: $u_name})-[r]->(o:Concept)
            WHERE type(r) <> 'INTERACTED_WITH'
            RETURN type(r) as rel, o.name as target
            LIMIT 20
        ";
        let mut result = self
            .graph
            .execute(query(search_q).param("u_name", user_name))
            .await?;
        let mut facts = Vec::new();

        while let Some(row) = result.next().await? {
            let rel: String = row.get("rel")?;
            let target: String = row.get("target")?;
            facts.push(format!("({}-{}-{})", user_name, rel, target));
        }

        Ok(if facts.is_empty() {
            "暂无已知事实".to_string()
        } else {
            facts.join(", ")
        })
    }

    /// 【增/改】存储或更新一个三元组事实
    async fn upsert_fact(&self, sub: &str, rel: &str, obj: &str) -> Result<()> {
        // 使用反引号包裹关系名以支持中文
        let cypher = format!(
            "MERGE (s {{name: $sub}}) 
             MERGE (o:Concept {{name: $obj}}) 
             MERGE (s)-[r:`{}`]->(o) 
             SET r.updated_at = timestamp()",
            rel
        );
        let q = query(&cypher).param("sub", sub).param("obj", obj);
        self.graph.run(q).await.map_err(|e| anyhow::anyhow!(e))
    }

    /// 【删】删除特定的关系
    async fn delete_fact(&self, sub: &str, rel: &str, obj: &str) -> Result<()> {
        let cypher = format!(
            "MATCH (s {{name: $sub}})-[r:`{}`]->(o {{name: $obj}}) DELETE r",
            rel
        );
        self.graph
            .run(query(&cypher).param("sub", sub).param("obj", obj))
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    // 删除节点及其所有关系
    async fn delete_node(&self, node_name: &str) -> Result<()> {
        Ok(())
    }
    /// 解析 AI 返回的 JSON 并批量应用到 Neo4j
    async fn apply_batch_update(graph: Graph, ai_json: &str) -> anyhow::Result<()> {
        // 1. 清洗并解析 JSON
        let cleaned = crate::utils::json_util::clean_json_string(ai_json);
        let update: MemoryUpdate = serde_json::from_str(cleaned)
            .map_err(|e| anyhow::anyhow!("JSON解析失败: {}, 内容: {}", e, cleaned))?;

        // 2. 批量执行指令
        for cmd in update.graph {
            if cmd.relation.trim().is_empty() {
                continue;
            }

            let cypher = match cmd.action.to_uppercase().as_str() {
                // UPSERT: 存在则更新时间戳，不存在则创建全链路
                "UPSERT" => format!(
                    "MERGE (s {{name: $sub}}) 
                     MERGE (o:Concept {{name: $obj}}) 
                     MERGE (s)-[r:`{}`]->(o) 
                     SET r.updated_at = timestamp()",
                    cmd.relation // 关系名动态嵌入，反引号支持中文
                ),
                // DELETE: 仅删除关系线，保留节点
                "DELETE" => format!(
                    "MATCH (s {{name: $sub}})-[r:`{}`]->(o:Concept {{name: $obj}}) 
                     DELETE r",
                    cmd.relation
                ),
                _ => continue,
            };

            // 3. 执行参数化查询
            let q = neo4rs::query(&cypher)
                .param("sub", cmd.subject.as_str())
                .param("obj", cmd.object.as_str());

            if let Err(e) = graph.run(q).await {
                error!(
                    "❌ 记忆指令执行失败: {} | 事实: ({}, {}, {})",
                    e, cmd.subject, cmd.relation, cmd.object
                );
            } else {
                debug!(
                    "✅ 记忆指令执行成功: [{}] ({} - {} -> {})",
                    cmd.action, cmd.subject, cmd.relation, cmd.object
                );
            }
        }
        Ok(())
    }
}

impl Actor for AgentMemoryHActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("🧠 记忆智能体启动成功，当前操作标识: {}", self.ai_name);
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FactCommand {
    pub action: String, // "UPSERT" 或 "DELETE"
    pub subject: String,
    pub relation: String,
    pub object: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct MemoryUpdate {
    pub graph: Vec<FactCommand>,
}

// --- 核心业务 Handler ---
impl Handler<RequestMemory> for AgentMemoryHActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: RequestMemory, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        let openai = self.open_aiproxy_actor.clone();
        let user_name = msg.user_name.clone();
        let user_content = msg.message_content.clone();
        let ai_name = self.ai_name.clone();
        let memory_prompt = self.agent_memory_prompt_template.clone();

        Box::pin(
            async move {

                // 结构化背景检索 ---
                let search_q = "
                MATCH (u:User {name: $u_name})
                OPTIONAL MATCH (u)-[r]->(o:Concept)
                RETURN collect(DISTINCT {rel: type(r), target: o.name}) as facts
                LIMIT 20
            ";
                let mut result = graph
                    .execute(query(search_q).param("u_name", user_name.clone()))
                    .await?;
                let mut knowledge_summary = String::from("暂无已知持久性事实");

                if let Some(row) = result.next().await? {
                    let facts: Vec<serde_json::Value> = row.get("facts").unwrap_or_default();

                    let fact_strings: Vec<String> = facts
                        .iter()
                        .filter_map(|f| {
                            let rel = f["rel"].as_str()?;
                            let target = f["target"].as_str()?;
                            // 拼写成你要求的 (banchen-KNOWS-rust) 格式
                            Some(format!("({}-{}-{})", user_name, rel, target))
                        })
                        .collect();

                    if !fact_strings.is_empty() {
                        knowledge_summary = fact_strings.join(", ");
                    }
                }

                let new_memory_prompt = memory_prompt
                    .replace("{ai_name}", &format!("{}", ai_name))
                    .replace("{user_name}", &format!("{}", user_name))
                    .replace("{knowledge_summary}", &format!("{}", knowledge_summary))
                    .replace("{user_content}", &format!("{}", user_content));

                // 发送反思请求
                if let Ok(Ok(ai_response)) = openai
                    .send(CallOpenAI {
                        prompt: new_memory_prompt,
                    })
                    .await
                {
                    tokio::spawn(async move {
                        if let Err(e) = Self::apply_batch_update(graph, &ai_response).await {
                            warn!("记忆批量更新任务失败: {}", e);
                        }
                    });
                }

                // 4. 返回背景
                Ok(format!(" {}\n", knowledge_summary))
            }
            .into_actor(self),
        )
    }
}
