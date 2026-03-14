use neo4rs::*;
use serde::{Deserialize, Serialize};
use tracing::debug;
use tracing::warn;
use std::collections::HashMap;
use std::fmt::Write;
use anyhow::Result;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: i64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct RelInfo {
    pub id: i64,
    pub start_node_id: i64,
    pub end_node_id: i64,
    pub rel_type: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ThreeLayerResult {
    pub nodes: Vec<NodeInfo>,
    pub relationships: Vec<RelInfo>,
}

/// 基于你提供的源码，通过 .value 字段访问数据
fn bolt_to_string(val: BoltType) -> String {
    match val {
        BoltType::String(s) => s.value,
        BoltType::Integer(i) => i.value.to_string(),
        BoltType::Float(f) => f.value.to_string(),
        BoltType::Boolean(b) => b.value.to_string(),
        BoltType::Null(_) => "null".to_string(),
        _ => format!("{:?}", val),
    }
}

pub async fn fetch_all_entities(
    graph: &Graph,
) -> Result<ThreeLayerResult, Box<dyn std::error::Error + Send + Sync>> {
    // Cypher 逻辑：
    // 1. MATCH (n) : 匹配图中所有的节点
    // 2. OPTIONAL MATCH (n)-[r]->(m) : 匹配这些节点发出的所有关系
    // 3. 使用同样的解构方式返回，确保兼容之前的解析逻辑
    let q = query(
        r#"
        MATCH (n)
        OPTIONAL MATCH (n)-[r]->(m)
        RETURN DISTINCT 
            id(n) AS n_id, 
            labels(n) AS n_labels, 
            properties(n) AS n_props,
            id(r) AS r_id,
            type(r) AS r_type,
            id(startNode(r)) AS r_start,
            id(endNode(r)) AS r_end,
            properties(r) AS r_props
        "#,
    );

    let mut result = graph.execute(q).await?;
    let mut node_map: HashMap<i64, NodeInfo> = HashMap::new();
    let mut rel_map: HashMap<i64, RelInfo> = HashMap::new();

    while let Some(row) = result.next().await? {
        // 1. 解析所有节点
        if let Ok(n_id) = row.get::<i64>("n_id") {
            node_map.entry(n_id).or_insert_with(|| {
                let n_labels: Vec<String> = row.get("n_labels").unwrap_or_default();
                let n_props: HashMap<String, BoltType> = row.get("n_props").unwrap_or_default();
                let mut properties = HashMap::new();
                for (k, v) in n_props {
                    properties.insert(k, bolt_to_string(v));
                }
                NodeInfo {
                    id: n_id,
                    labels: n_labels,
                    properties,
                }
            });
        }

        // 2. 解析所有关系
        if let Ok(r_id) = row.get::<i64>("r_id") {
            rel_map.entry(r_id).or_insert_with(|| {
                let r_type: String = row.get("r_type").unwrap_or_else(|_| "UNKNOWN".to_string());
                let r_start: i64 = row.get("r_start").unwrap_or(-1);
                let r_end: i64 = row.get("r_end").unwrap_or(-1);
                let r_props: HashMap<String, BoltType> = row.get("r_props").unwrap_or_default();
                let mut properties = HashMap::new();
                for (k, v) in r_props {
                    properties.insert(k, bolt_to_string(v));
                }
                RelInfo {
                    id: r_id,
                    start_node_id: r_start,
                    end_node_id: r_end,
                    rel_type: r_type,
                    properties,
                }
            });
        }
    }

    Ok(ThreeLayerResult {
        nodes: node_map.into_values().collect(),
        relationships: rel_map.into_values().collect(),
    })
}

pub fn format_graph_to_string(result: &ThreeLayerResult) -> String {
    let mut out = String::new();

    // 1. 建立 ID 到节点的快速索引
    let node_map: HashMap<i64, &NodeInfo> = result.nodes.iter().map(|n| (n.id, n)).collect();

    // 生成分割线
    let line = "=".repeat(50);

    // 使用 writeln! 写入缓冲区，.unwrap() 是因为向 String 写入几乎不会失败
    writeln!(out, "\n{}", line).unwrap();
    writeln!(
        out,
        "🔍 图谱路径详细信息 ({} 节点, {} 关系)",
        result.nodes.len(),
        result.relationships.len()
    )
    .unwrap();
    writeln!(out, "{}", line).unwrap();

    // 2. 处理关系路径
    if result.relationships.is_empty() {
        writeln!(out, "(该节点暂无关联关系)").unwrap();
    } else {
        for rel in &result.relationships {
            let start_node = node_map.get(&rel.start_node_id);
            let end_node = node_map.get(&rel.end_node_id);

            let start_name = start_node
                .and_then(|n| n.properties.get("name"))
                .cloned()
                .unwrap_or_else(|| format!("ID:{}", rel.start_node_id));
            let end_name = end_node
                .and_then(|n| n.properties.get("name"))
                .cloned()
                .unwrap_or_else(|| format!("ID:{}", rel.end_node_id));

            writeln!(
                out,
                "\n🚀 路径: {} -[:{}]-> {}",
                start_name, rel.rel_type, end_name
            )
            .unwrap();

            if let Some(n) = start_node {
                writeln!(
                    out,
                    "   ├─ [源节点] 标签:{:?}, 属性: {:?}",
                    n.labels, n.properties
                )
                .unwrap();
            }

            if rel.properties.is_empty() {
                writeln!(out, "   ├─ [关系属性] (无)").unwrap();
            } else {
                writeln!(out, "   ├─ [关系属性] {:?}", rel.properties).unwrap();
            }

            if let Some(n) = end_node {
                writeln!(
                    out,
                    "   └─ [目标节点] 标签:{:?}, 属性: {:?}",
                    n.labels, n.properties
                )
                .unwrap();
            }
        }
    }

    // 3. 处理孤立节点
    let mut connected_ids = std::collections::HashSet::new();
    for rel in &result.relationships {
        connected_ids.insert(rel.start_node_id);
        connected_ids.insert(rel.end_node_id);
    }

    let lonely_nodes: Vec<&NodeInfo> = result
        .nodes
        .iter()
        .filter(|n| !connected_ids.contains(&n.id))
        .collect();

    if !lonely_nodes.is_empty() {
        writeln!(out, "\n📍 孤立节点 (无连接):").unwrap();
        for node in lonely_nodes {
            writeln!(
                out,
                "   • {} (标签:{:?}, 属性:{:?})",
                node.properties
                    .get("name")
                    .unwrap_or(&format!("ID:{}", node.id)),
                node.labels,
                node.properties
            )
            .unwrap();
        }
    }
    writeln!(out, "\n{}", line).unwrap();

    out // 返回最终生成的字符串
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
