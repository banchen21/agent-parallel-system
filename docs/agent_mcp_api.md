# 智能体 MCP 管理 API 文档

## 概述

本文档描述了智能体 MCP（Model Context Protocol）工具管理的 API 接口。智能体可以通过这些接口添加、删除和查询可用的 MCP 工具。

## 数据库变更

### agents 表新增字段

```sql
ALTER TABLE agents ADD COLUMN IF NOT EXISTS mcp_list TEXT[] DEFAULT '{}';
```

- `mcp_list`: 存储智能体可用的 MCP 工具名称列表（字符串数组）

## API 接口

所有接口都需要在请求头中携带 JWT Token：
```
Authorization: Bearer <your_jwt_token>
```

### 1. 创建智能体

**接口**: `POST /api/v1/agent`

**请求体**:
```json
{
  "name": "智能体名称",
  "kind": "general",
  "workspace_name": "工作区名称"
}
```

**响应**:
```json
{
  "id": "uuid",
  "name": "智能体名称",
  "kind": "general",
  "workspace_name": "工作区名称",
  "mcp_list": []
}
```

### 2. 查询智能体信息

**接口**: `GET /api/v1/agent/{agent_id}`

**路径参数**:
- `agent_id`: 智能体的 UUID

**响应**:
```json
{
  "id": "uuid",
  "name": "智能体名称",
  "kind": "general",
  "workspace_name": "工作区名称",
  "mcp_list": ["postgres", "filesystem"]
}
```

### 3. 为智能体添加 MCP 工具

**接口**: `POST /api/v1/agent/{agent_id}/mcp`

**路径参数**:
- `agent_id`: 智能体的 UUID

**请求体**:
```json
{
  "mcp_name": "postgres"
}
```

**响应**:
```json
{
  "id": "uuid",
  "name": "智能体名称",
  "kind": "general",
  "workspace_name": "工作区名称",
  "mcp_list": ["postgres"]
}
```

**错误响应**:
- 如果 MCP 已存在：`400 Bad Request - "MCP postgres 已存在于智能体的工具列表中"`
- 如果智能体不存在：`400 Bad Request - "智能体 {id} 不存在"`

### 4. 从智能体移除 MCP 工具

**接口**: `DELETE /api/v1/agent/{agent_id}/mcp/{mcp_name}`

**路径参数**:
- `agent_id`: 智能体的 UUID
- `mcp_name`: MCP 工具名称

**响应**:
```json
{
  "id": "uuid",
  "name": "智能体名称",
  "kind": "general",
  "workspace_name": "工作区名称",
  "mcp_list": []
}
```

## 使用示例

### 示例 1: 创建智能体并添加 MCP 工具

```bash
# 1. 创建智能体
curl -X POST http://localhost:8000/api/v1/agent \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "数据分析助手",
    "kind": "general",
    "workspace_name": "myworkspace"
  }'

# 响应: {"id": "123e4567-e89b-12d3-a456-426614174000", ...}

# 2. 为智能体添加 postgres MCP 工具
curl -X POST http://localhost:8000/api/v1/agent/123e4567-e89b-12d3-a456-426614174000/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "mcp_name": "postgres"
  }'

# 3. 再添加 filesystem MCP 工具
curl -X POST http://localhost:8000/api/v1/agent/123e4567-e89b-12d3-a456-426614174000/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "mcp_name": "filesystem"
  }'
```

### 示例 2: 查询智能体的 MCP 工具列表

```bash
curl -X GET http://localhost:8000/api/v1/agent/123e4567-e89b-12d3-a456-426614174000 \
  -H "Authorization: Bearer YOUR_TOKEN"

# 响应:
# {
#   "id": "123e4567-e89b-12d3-a456-426614174000",
#   "name": "数据分析助手",
#   "kind": "general",
#   "workspace_name": "myworkspace",
#   "mcp_list": ["postgres", "filesystem"]
# }
```

### 示例 3: 移除 MCP 工具

```bash
curl -X DELETE http://localhost:8000/api/v1/agent/123e4567-e89b-12d3-a456-426614174000/mcp/postgres \
  -H "Authorization: Bearer YOUR_TOKEN"

# 响应:
# {
#   "id": "123e4567-e89b-12d3-a456-426614174000",
#   "name": "数据分析助手",
#   "kind": "general",
#   "workspace_name": "myworkspace",
#   "mcp_list": ["filesystem"]
# }
```

## 常见 MCP 工具

根据 `.roo/mcp.json` 配置，系统支持以下 MCP 工具：

- `postgres`: PostgreSQL 数据库操作工具
- `Apifox 开放 API - API 文档`: Apifox API 文档工具

智能体可以根据需要添加这些工具来扩展其能力。

## 注意事项

1. **MCP 工具名称必须与 `.roo/mcp.json` 中配置的名称一致**
2. 添加重复的 MCP 工具会返回错误
3. 移除不存在的 MCP 工具不会报错，会静默处理
4. 智能体的 MCP 列表存储在数据库中，重启后会保持
5. 所有接口都需要认证，未认证的请求会返回 401 错误

## 架构说明

### Actor 模式

系统使用 Actix Actor 模式管理智能体和 MCP 工具：

- **AgentActor**: 管理所有智能体的状态和操作
- **消息类型**:
  - `CreateAgent`: 创建新智能体
  - `GetAgentInfo`: 查询智能体信息
  - `AddMcpToAgent`: 为智能体添加 MCP 工具
  - `RemoveMcpFromAgent`: 从智能体移除 MCP 工具

### 数据流

```
HTTP Request → Handler → AgentActor → Database → Response
```

1. HTTP 请求到达 handler
2. Handler 将请求转换为 Actor 消息
3. AgentActor 处理消息并操作数据库
4. 返回结果给 handler
5. Handler 将结果转换为 HTTP 响应
