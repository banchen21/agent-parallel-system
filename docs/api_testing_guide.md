# API 测试指南

本文档基于当前实际后端接口，给出最小可复现的测试顺序。

## 前提

启动服务：

```bash
cargo run
```

默认监听地址：`http://localhost:8000`

## 1. 注册用户

```bash
curl -X POST http://localhost:8000/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "banchen",
    "password": "123456",
    "email": "banchen@example.com"
  }'
```

说明：

1. 注册成功后，系统会异步创建默认工作区
2. 并尝试创建一个默认执行型 Agent

## 2. 登录获取 Token

```bash
curl -X POST http://localhost:8000/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "banchen",
    "password": "123456"
  }'
```

返回：

```json
{
  "access_token": "...",
  "refresh_token": "..."
}
```

后续测试统一使用：

```bash
export ACCESS_TOKEN="你的 access token"
export REFRESH_TOKEN="你的 refresh token"
```

## 3. 查询工作区

```bash
curl http://localhost:8000/api/v1/workspace \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

## 4. 查询 Agent 列表

```bash
curl http://localhost:8000/api/v1/agent \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

## 5. 创建任务

当前任务创建接口请求体是 `TaskItem` 结构，`id` 字段会被接收但不会作为最终任务 ID 使用，可传空字符串。

```bash
curl -X POST http://localhost:8000/api/v1/tasks \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "",
    "name": "检查当前目录文件",
    "description": "列出当前工作目录下的文件并总结结果",
    "priority": "medium",
    "depends_on": []
  }'
```

返回状态通常是 `202 Accepted`。

## 6. 查询任务列表

```bash
curl http://localhost:8000/api/v1/tasks \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

## 7. 获取单个任务详情

```bash
curl http://localhost:8000/api/v1/tasks/<task_id> \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

## 8. 处理审阅决策

```bash
curl -X POST http://localhost:8000/api/v1/tasks/<task_id>/review-decision \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"accept": true}'
```

## 9. 测试 MCP 工具 CRUD

### 查询工具

```bash
curl http://localhost:8000/api/v1/mcp/tools \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

### 创建工具

```bash
curl -X POST http://localhost:8000/api/v1/mcp/tools \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "toolId": "echo_tool",
    "description": "执行 echo 命令",
    "parameters": {
      "type": "object",
      "properties": {
        "command": {"type": "string", "description": "shell 命令"}
      },
      "required": ["command"]
    },
    "options": {"timeoutMs": 30000, "maxRetries": 1},
    "execution": {
      "transport": "builtin",
      "endpoint": "terminal_run",
      "method": "POST",
      "headers": {}
    }
  }'
```

## 10. 测试 WebSocket 聊天

连接地址：

```text
ws://localhost:8000/ws/chat?token=<access_token>
```

客户端发送：

```json
{
  "content": "帮我创建一个任务：列出当前目录文件",
  "device_type": "web"
}
```

## 11. 测试日志流

```text
GET /logs/stream?token=<access_token>
```

## 推荐测试顺序

1. 注册
2. 登录
3. 查询工作区和 Agent
4. 创建任务
5. 观察任务状态变化
6. 审阅决策
7. MCP 工具 CRUD
8. WebSocket 与 SSE
