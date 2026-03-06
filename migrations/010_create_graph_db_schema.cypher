// Neo4j Cypher 脚本 - 创建记忆图数据库结构

// 创建约束
CREATE CONSTRAINT IF NOT EXISTS FOR (n:Entity) REQUIRE n.id IS UNIQUE;
CREATE CONSTRAINT IF NOT EXISTS FOR (n:Concept) REQUIRE n.id IS UNIQUE;
CREATE CONSTRAINT IF NOT EXISTS FOR (n:Event) REQUIRE n.id IS UNIQUE;
CREATE CONSTRAINT IF NOT EXISTS FOR (n:Session) REQUIRE n.id IS UNIQUE;

// 创建索引
CREATE INDEX IF NOT EXISTS FOR (n:Entity) ON (n.label);
CREATE INDEX IF NOT EXISTS FOR (n:Concept) ON (n.label);
CREATE INDEX IF NOT EXISTS FOR (n:Event) ON (n.timestamp);
CREATE INDEX IF NOT EXISTS FOR (n:Session) ON (n.created_at);

// 创建示例节点类型
// Entity - 实体节点（人物、地点、物品等）
// Concept - 概念节点（想法、主题等）
// Event - 事件节点（对话、交互等）
// Session - 会话节点（聊天会话）

// 创建示例关系类型
// MENTIONS - 提及关系
// RELATED_TO - 相关关系
// RESPONSE_TO - 回复关系
// CONTAINS - 包含关系
// SIMILAR_TO - 相似关系
// CONTRADICTS - 矛盾关系
