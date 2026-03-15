# MCP 配置管理 API 文档

## 概述

本文档描述了 MCP（Model Context Protocol）配置管理的 API 接口。系统提供了完整的 MCP 配置文件管理功能，包括添加、删除、查询 MCP 配置。

## 目录结构

```
.mcps/                    # MCP 配置文件目录
  ├── README.md          # 说明文档
  ├── postgres.json      # PostgreSQL MCP 配置
  └── filesystem.json    # 文件系统 MCP 配置

.roo/
  └── mcp.json           # MCP 配置汇总文件（自动生成）
```

## API 接口

所有接口都需要在请求头中携带 JWT Token：
```
Authorization: Bearer <your_jwt_token>
```

### 1. 添加 MCP 配置

**接口**: `POST /api/v1/mcp`

**请求体**:
```json
{
  "name": "postgres",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-postgres-server"],
  "env": {
    "PG_HOST": "127.0.0.1",
    "PG_PORT": "5432",
    "PG_USER": "postgres",
    "PG_PASSWORD": "password",
    "PG_DATABASE": "agent_system"
  },
  "alwaysAllow": ["list_tables"]
}
```

**字段说明**:
- `name`: MCP 名称（必填，唯一标识）
- `type`: MCP 类型（可选，默认 "stdio"）
- `command`: 执行命令（必填）
- `args`: 命令参数（可选，数组）
- `env`: 环境变量（可选，键值对）
- `alwaysAllow`: 始终允许的操作（可选，数组）

**响应**:
```json
{
  "name": "postgres",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-postgres-server"],
  "env": {
    "PG_HOST": "127.0.0.1",
    "PG_PORT": "5432",
    "PG_USER": "postgres",
    "PG_PASSWORD": "password",
    "PG_DATABASE": "agent_system"
  },
  "alwaysAllow": ["list_tables"]
}
```

**错误响应**:
- `400 Bad Request`: MCP 配置已存在或配置无效
- `500 Internal Server Error`: 服务器内部错误

### 2. 删除 MCP 配置

**接口**: `DELETE /api/v1/mcp/{name}`

**路径参数**:
- `name`: MCP 名称

**响应**:
```json
{
  "status": "success",
  "message": "MCP 配置删除成功"
}
```

**错误响应**:
- `400 Bad Request`: MCP 配置不存在
- `500 Internal Server Error`: 服务器内部错误

### 3. 查询单个 MCP 配置

**接口**: `GET /api/v1/mcp/{name}`

**路径参数**:
- `name`: MCP 名称

**响应**:
```json
{
  "name": "postgres",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-postgres-server"],
  "env": {
    "PG_HOST": "127.0.0.1",
    "PG_PORT": "5432",
    "PG_USER": "postgres",
    "PG_PASSWORD": "password",
    "PG_DATABASE": "agent_system"
  },
  "alwaysAllow": ["list_tables"]
}
```

**错误响应**:
- `404 Not Found`: MCP 配置不存在
- `500 Internal Server Error`: 服务器内部错误

### 4. 查询所有 MCP 配置

**接口**: `GET /api/v1/mcp`

**响应**:
```json
[
  {
    "name": "postgres",
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "mcp-postgres-server"],
    "env": {
      "PG_HOST": "127.0.0.1",
      "PG_PORT": "5432",
      "PG_USER": "postgres",
      "PG_PASSWORD": "password",
      "PG_DATABASE": "agent_system"
    },
    "alwaysAllow": ["list_tables"]
  },
  {
    "name": "filesystem",
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "mcp-filesystem-server"],
    "env": {},
    "alwaysAllow": []
  }
]
```

## 使用示例

### 示例 1: 添加 PostgreSQL MCP 配置

```bash
curl -X POST http://localhost:8000/api/v1/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "postgres",
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "mcp-postgres-server"],
    "env": {
      "PG_HOST": "127.0.0.1",
      "PG_PORT": "5432",
      "PG_USER": "postgres",
      "PG_PASSWORD": "password",
      "PG_DATABASE": "agent_system"
    },
    "alwaysAllow": ["list_tables"]
  }'
```

### 示例 2: 添加文件系统 MCP 配置

```bash
curl -X POST http://localhost:8000/api/v1/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "filesystem",
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "mcp-filesystem-server"],
    "env": {},
    "alwaysAllow": ["read_file", "write_file"]
  }'
```

### 示例 3: 查询所有 MCP 配置

```bash
curl -X GET http://localhost:8000/api/v1/mcp \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### 示例 4: 查询单个 MCP 配置

```bash
curl -X GET http://localhost:8000/api/v1/mcp/postgres \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### 示例 5: 删除 MCP 配置

```bash
curl -X DELETE http://localhost:8000/api/v1/mcp/postgres \
  -H "Authorization: Bearer YOUR_TOKEN"
```

## 与智能体集成

添加 MCP 配置后，可以将其分配给智能体使用：

```bash
# 1. 添加 MCP 配置
curl -X POST http://localhost:8000/api/v1/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "postgres",
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "mcp-postgres-server"],
    "env": {
      "PG_HOST": "127.0.0.1",
      "PG_PORT": "5432",
      "PG_USER": "postgres",
      "PG_PASSWORD": "password",
      "PG_DATABASE": "agent_system"
    }
  }'

# 2. 为智能体添加 MCP 工具
curl -X POST http://localhost:8000/api/v1/agent/{agent_id}/mcp \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "mcp_name": "postgres"
  }'
```

## 工作流程

1. **添加 MCP 配置**:
   - 配置文件保存到 `.mcps/{name}.json`
   - 同时更新 `.roo/mcp.json` 汇总文件

2. **删除 MCP 配置**:
   - 删除 `.mcps/{name}.json` 文件
   - 从 `.roo/mcp.json` 中移除对应配置

3. **查询 MCP 配置**:
   - 从 `.mcps/` 目录读取配置文件
   - 或从 `.roo/mcp.json` 读取汇总信息

## 架构说明

### Actor 模式

系统使用 Actix Actor 模式管理 MCP 配置：

- **McpManagerActor**: 管理所有 MCP 配置的状态和操作
- **消息类型**:
  - `AddMcpConfig`: 添加 MCP 配置
  - `DeleteMcpConfig`: 删除 MCP 配置
  - `GetMcpConfig`: 查询单个 MCP 配置
  - `ListMcpConfigs`: 查询所有 MCP 配置

### 数据流

```
HTTP Request → Handler → McpManagerActor → File System → Response
```

1. HTTP 请求到达 handler
2. Handler 将请求转换为 Actor 消息
3. McpManagerActor 处理消息并操作文件系统
4. 返回结果给 handler
5. Handler 将结果转换为 HTTP 响应

## 注意事项

1. **配置文件格式**: 所有配置文件必须是有效的 JSON 格式
2. **名称唯一性**: MCP 名称必须唯一，不能重复
3. **文件同步**: `.mcps/` 目录和 `.roo/mcp.json` 会自动同步
4. **权限控制**: 所有接口都需要 JWT 认证
5. **环境变量**: 敏感信息（如密码）应通过环境变量配置

## 常见 MCP 工具配置示例

### PostgreSQL

```json
{
  "name": "postgres",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-postgres-server"],
  "env": {
    "PG_HOST": "127.0.0.1",
    "PG_PORT": "5432",
    "PG_USER": "postgres",
    "PG_PASSWORD": "password",
    "PG_DATABASE": "agent_system"
  },
  "alwaysAllow": ["list_tables"]
}
```

### 文件系统

```json
{
  "name": "filesystem",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-filesystem-server"],
  "env": {},
  "alwaysAllow": ["read_file", "write_file", "list_directory"]
}
```

### Git

```json
{
  "name": "git",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "mcp-git-server"],
  "env": {},
  "alwaysAllow": ["git_status", "git_log"]
}
```
