use uuid::Uuid;
use serde_json::{json, Value};

use crate::core::errors::AppError;
use super::graph_db::{GraphDBClient, GraphNode, GraphEdge, MemoryGraphBuilder};

/// 记忆管理服务
pub struct MemoryService {
    graph_db: GraphDBClient,
}

impl MemoryService {
    pub fn new(graph_db_endpoint: String, api_key: Option<String>) -> Self {
        Self {
            graph_db: GraphDBClient::new(graph_db_endpoint, api_key),
        }
    }

    /// 保存对话到记忆图
    pub async fn save_conversation_to_memory(
        &self,
        session_id: Uuid,
        user_message: &str,
        assistant_message: &str,
    ) -> Result<(), AppError> {
        let mut builder = MemoryGraphBuilder::new();

        // 创建用户消息节点
        let user_msg_id = Uuid::new_v4().to_string();
        builder.extract_from_message(Uuid::parse_str(&user_msg_id).unwrap(), user_message)?;

        // 创建助手消息节点
        let assistant_msg_id = Uuid::new_v4().to_string();
        builder.extract_from_message(Uuid::parse_str(&assistant_msg_id).unwrap(), assistant_message)?;

        // 创建会话节点
        let session_node = GraphNode {
            id: session_id.to_string(),
            node_type: "event".to_string(),
            label: format!("Session {}", session_id),
            properties: json!({
                "type": "conversation",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
            embedding: None,
        };
        builder.add_node(session_node);

        // 创建消息之间的关系
        let edge = GraphEdge {
            id: Uuid::new_v4().to_string(),
            source_id: user_msg_id.clone(),
            target_id: assistant_msg_id.clone(),
            relation_type: "response_to".to_string(),
            properties: json!({}),
            weight: 1.0,
        };
        builder.add_edge(edge);

        // 创建会话和消息的关系
        let session_edge = GraphEdge {
            id: Uuid::new_v4().to_string(),
            source_id: session_id.to_string(),
            target_id: user_msg_id,
            relation_type: "contains".to_string(),
            properties: json!({}),
            weight: 1.0,
        };
        builder.add_edge(session_edge);

        Ok(())
    }

    /// 从记忆中检索相关信息
    pub async fn retrieve_relevant_memory(
        &self,
        session_id: Uuid,
        _query: &str,
        depth: i32,
    ) -> Result<Vec<String>, AppError> {
        // 查询相关节点
        let related_nodes = self
            .graph_db
            .query_related_nodes(&session_id.to_string(), depth)
            .await?;

        // 提取相关信息
        let relevant_info: Vec<String> = related_nodes
            .iter()
            .map(|node| node.label.clone())
            .collect();

        Ok(relevant_info)
    }

    /// 提取实体和关系
    pub async fn extract_entities_and_relations(
        &self,
        _text: &str,
    ) -> Result<(Vec<String>, Vec<(String, String, String)>), AppError> {
        // 这里实现 NLP 提取逻辑
        // 返回 (实体列表, 关系列表)
        
        let entities = vec![];
        let relations = vec![];

        Ok((entities, relations))
    }

    /// 更新记忆中的实体
    pub async fn update_entity_memory(
        &self,
        entity_id: &str,
        properties: Value,
    ) -> Result<(), AppError> {
        self.graph_db.update_node(entity_id, properties).await?;
        Ok(())
    }

    /// 获取会话的记忆摘要
    pub async fn get_session_memory_summary(
        &self,
        session_id: Uuid,
    ) -> Result<Value, AppError> {
        let related_nodes = self
            .graph_db
            .query_related_nodes(&session_id.to_string(), 2)
            .await?;

        let summary = json!({
            "session_id": session_id,
            "entities_count": related_nodes.len(),
            "entities": related_nodes.iter().map(|n| {
                json!({
                    "id": n.id,
                    "type": n.node_type,
                    "label": n.label,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(summary)
    }

    /// 清理过期记忆
    pub async fn cleanup_old_memory(&self, _days: i32) -> Result<u64, AppError> {
        // 这里实现清理逻辑
        Ok(0)
    }
}
