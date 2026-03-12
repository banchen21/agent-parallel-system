use neo4rs::*;
use std::collections::HashMap;
use std::fmt::Write;
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
