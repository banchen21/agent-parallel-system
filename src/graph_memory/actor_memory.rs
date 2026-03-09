use actix::prelude::*;
use anyhow::Result;
use neo4rs::{Graph, query};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn, debug};

// 引用之前定义的 OpenAI Actor 和 错误类型
use crate::{chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor}, utils::json_util::clean_json_string};

// --- 记忆指令结构（用于解析 AI 的决策） ---

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct MemoryUpdate {
    pub commands: Vec<String>, // AI 生成的 Cypher MERGE/SET 语句
}

// --- Actix 消息定义 ---



// --- Actor 定义 ---

pub struct AgentMemoryHActor {
    pub graph: Graph,
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
}

impl AgentMemoryHActor {
    pub async fn new(uri: &str, user: &str, pass: &str, open_aiproxy_actor: Addr<OpenAIProxyActor>) -> Result<Self> {
        let graph = Graph::new(uri, user, pass).await?;
        Ok(Self {
            graph,
            open_aiproxy_actor,
        })
    }

    /// 内部逻辑：将 AI 提取的知识持久化到 Neo4j
    async fn persist_ai_thoughts(graph: Arc<Graph>, ai_json: String) -> Result<()> {
        // 【关键修复】复用清洗逻辑，处理 AI 可能返回的 ```json 标签
        let cleaned_json = clean_json_string(&ai_json);
        
        debug!("尝试持久化记忆指令: {}", cleaned_json);

        if let Ok(update) = serde_json::from_str::<MemoryUpdate>(cleaned_json) {
            for statement in update.commands {
                if !statement.is_empty() {
                    // 过滤掉 AI 可能生成的非法注释或空行
                    if statement.trim().is_empty() { continue; }
                    
                    let q = query(&statement);
                    if let Err(e) = graph.run(q).await {
                        error!("❌ 自主记忆写入失败: {}, 语句: {}", e, statement);
                    } else {
                        debug!("✅ 记忆指令执行成功: {}", statement);
                    }
                }
            }
        } else {
            warn!("⚠️ 记忆智能体解析 AI 指令失败，AI 可能没有按 JSON 格式输出或输出为空");
        }
        Ok(())
    }

}

impl Actor for AgentMemoryHActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("🧠 记忆管理智能体 (AgentMemoryHActor) 已启动");
    }
}


/// 唯一的入口消息：ChatAgent 只传这两个字段
#[derive(Message)]
#[rtype(result = "Result<String, ChatAgentError>")]
pub struct RequestMemory {
    pub user_name: String,
    pub message_content: String,
}

impl Handler<RequestMemory> for AgentMemoryHActor {
    type Result = ResponseActFuture<Self, Result<String, ChatAgentError>>;

    fn handle(&mut self, msg: RequestMemory, _ctx: &mut Self::Context) -> Self::Result {
        let graph = self.graph.clone();
        let openai = self.open_aiproxy_actor.clone();
        let user_name = msg.user_name.clone();
        let user_content = msg.message_content.clone();

        Box::pin(
            async move {
                debug!("开始为用户 {} 检索/更新认知...", user_name);

                // 1. 【感知与初始化】 确保用户节点存在 (无则创建，有则更新活跃时间)
                let init_q = "
                    MERGE (u:User {name: $name})
                    ON CREATE SET u.created_at = timestamp(), u.first_interaction = $content
                    ON MATCH SET u.last_seen = timestamp()
                    RETURN u
                ";
                debug!("执行初始化语句: {}", init_q);
                graph.run(query(init_q).param("name", user_name.clone()).param("content", user_content.clone())).await
                    .map_err(|e| ChatAgentError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

                // 2. 【脑内检索】 获取当前用户的所有相关背景（标签、属性、关联实体）
                let search_q = "
                    MATCH (u:User {name: $name})
                    OPTIONAL MATCH (u)-[r]->(n)
                    RETURN labels(n) as tags, n.name as entity_name, type(r) as relationship, n.content as fact
                    LIMIT 10
                ";
                debug!("执行检索语句: {}", search_q);
                let mut result = graph.execute(query(search_q).param("name", user_name.clone())).await
                    .map_err(|e| ChatAgentError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

                let mut current_knowledge = Vec::new();
                while let Some(row) = result.next().await.unwrap_or(None) {
                    let entity: Option<String> = row.get("entity_name").ok();
                    let rel: Option<String> = row.get("relationship").ok();
                    if let (Some(e), Some(r)) = (entity, rel) {
                        current_knowledge.push(format!("(用户)-[{}]->({})", r, e));
                    }
                }
                
                debug!("当前认知: {:#?}", current_knowledge);
                let knowledge_str = if current_knowledge.is_empty() { 
                    "目前对他一无所知".to_string() 
                } else { 
                    current_knowledge.join(", ") 
                };

                // 3. 【认知决策】 调用 OpenAI 判定是否需要“学习”新内容
                let memory_prompt = format!(
                    "你是一个记忆管理员。
                    【当前认知】: 用户名为 '{}'，已知关系: {}。
                    【当前输入】: '{}'。
                    请分析输入，如果发现了新的实体（人名、项目、偏好、习惯、技能），请生成对应的 Cypher MERGE 语句。
                    如果是已有信息的修改，生成 SET 语句。
                    
                    输出格式要求 (JSON):
                    {{
                        \"commands\": [
                            \"MATCH (u:User {{name: '...'}}) MERGE (u)-[:WORKS_ON]->(p:Project {{name: '...'}})\",
                            \"MATCH (u:User {{name: '...'}}) SET u.hobby = '...'\"
                        ]
                    }}
                    如果无需更新，请保持 commands 为空数组。只输出 JSON。",
                    user_name, knowledge_str, user_content
                );

                debug!("预览反思:{}", memory_prompt);
                if let Ok(Ok(ai_thought)) = openai.send(CallOpenAI { prompt: memory_prompt }).await {
                    // 自主执行 AI 的决策（持久化到 Neo4j）
                    let _ = Self::persist_ai_thoughts(graph.into(), ai_thought).await;
                }

                // 4.  再次整合最新背景回传给 ChatAgent
                let final_context = format!(
                    "【关于 {} 的记忆背景】: {} \n【当前输入分析】: 用户提到了 '{}'，记忆已同步。",
                    user_name, knowledge_str, user_content
                );

                Ok(final_context) 
            }
            .into_actor(self),
        )
    }
}

