# Agent Parallel System API 规范

更新时间：2026-03-05

## 1. 基础信息

- **服务地址**：`http://127.0.0.1:8000`
- **API 前缀**：`/api/v1`
- **路由兼容**：同一接口同时支持"根路径"和"`/api/v1` 前缀"两种访问方式
- **Web 控制台**：`/`、`/docs`
- **机器可读接口清单**：`/ui/spec`
- **机器可读接口列表**：`/ui/endpoints`

建议生产环境统一使用带前缀地址，例如：`/api/v1/tasks`。

## 2. 认证方式

除健康检查、UI 文档接口、登录/注册外，其余接口要求：

```
Authorization: Bearer <access_token>
```

Token 获取流程：

1. `POST /auth/register` 注册
2. `POST /auth/login` 登录拿到 `access_token` 与 `refresh_token`
3. `POST /auth/refresh` 刷新访问令牌

## 3. 统一响应格式

### 3.1 成功响应

```json
{
  "success": true,
  "message": "任务创建成功",
  "data": {},
  "timestamp": "2026-03-05T16:00:00Z"
}
```

### 3.2 失败响应

```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "参数错误"
  },
  "timestamp": "2026-03-05T16:00:00Z"
}
```

## 4. 错误码

| HTTP | code | 说明 |
|---|---|---|
| 400 | `VALIDATION_ERROR` | 参数校验失败 |
| 400 | `JSON_ERROR` | JSON 解析/序列化失败 |
| 400 | `SERIALIZATION_ERROR` | 请求体结构错误 |
| 401 | `AUTHENTICATION_FAILED` | 认证失败或令牌无效 |
| 403 | `PERMISSION_DENIED` | 无权限访问资源 |
| 404 | `NOT_FOUND` | 资源不存在 |
| 429 | `RATE_LIMIT_EXCEEDED` | 请求过于频繁 |
| 500 | `DATABASE_ERROR` | 数据库操作失败 |
| 500 | `REDIS_ERROR` | 缓存/消息队列错误 |
| 500 | `TASK_EXECUTION_ERROR` | 任务执行错误 |
| 500 | `AGENT_ERROR` | 智能体处理错误 |
| 500 | `INTERNAL_SERVER_ERROR` / `INTERNAL_ERROR` | 其他内部错误 |
| 502 | `EXTERNAL_API_ERROR` | 外部依赖服务错误 |

## 5. 字段约定

- `id`、`*_id` 类型均为 UUID。
- 时间字段均为 ISO 8601（UTC），例如 `2026-03-05T15:59:59Z`。
- `TaskStatus`：`pending`、`in_progress`、`completed`、`failed`、`cancelled`。
- `TaskPriority`：`low`、`medium`、`high`、`urgent`。
- `AgentStatus`：`online`、`offline`、`busy`、`idle`、`error`。
- `PermissionLevel`：`read`、`write`、`admin`。

## 6. 接口总览

### 6.1 系统与文档

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/health` | 否 | 存活检查 |
| GET | `/ready` | 否 | 就绪检查 |
| GET | `/` | 否 | Web 控制台 |
| GET | `/docs` | 否 | Web 控制台 |
| GET | `/ui/endpoints` | 否 | 接口列表 JSON |
| GET | `/ui/spec` | 否 | API 规范 JSON |

### 6.2 认证 Auth

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/auth/register` | 否 | 用户注册 |
| POST | `/auth/login` | 否 | 用户登录 |
| POST | `/auth/refresh` | 否 | 刷新访问令牌 |
| POST | `/auth/logout` | 是 | 用户登出 |
| GET | `/auth/me` | 是 | 获取当前用户 |
| POST | `/auth/change-password` | 是 | 修改密码 |

### 6.3 任务 Task

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/tasks` | 是 | 创建任务（创建后立即尝试自动分配） |
| GET | `/tasks` | 是 | 查询任务列表 |
| GET | `/tasks/{task_id}` | 是 | 查询任务详情 |
| PUT | `/tasks/{task_id}` | 是 | 更新任务 |
| DELETE | `/tasks/{task_id}` | 是 | 删除任务 |
| PUT | `/tasks/{task_id}/status` | 是 | 更新任务状态 |
| POST | `/tasks/{task_id}/decompose` | 是 | 任务分解 |
| GET | `/tasks/{task_id}/subtasks` | 是 | 查询子任务 |

`GET /tasks` 查询参数：

- `workspace_id`（必填）
- `status`（可选）
- `priority`（可选）
- `page`（可选，默认 1）
- `page_size`（可选，默认 20）

### 6.4 智能体 Agent

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/agents` | 是 | 查询可用智能体 |
| POST | `/agents` | 是 | 注册智能体 |
| GET | `/agents/{agent_id}` | 是 | 智能体详情 |
| POST | `/agents/{agent_id}/heartbeat` | 是 | 上报心跳 |
| PUT | `/agents/{agent_id}/status` | 是 | 更新状态 |
| POST | `/agents/{agent_id}/assign-task` | 是 | 指定任务分配 |
| POST | `/agents/{agent_id}/complete-task` | 是 | 上报任务完成并释放负载 |
| GET | `/agents/stats` | 是 | 获取智能体统计 |

`GET /agents` 查询参数：

- `capabilities`（可选，逗号分隔）

### 6.5 工作空间 Workspace

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/workspaces` | 是 | 创建工作空间 |
| GET | `/workspaces` | 是 | 工作空间列表 |
| GET | `/workspaces/{workspace_id}` | 是 | 工作空间详情 |
| PUT | `/workspaces/{workspace_id}` | 是 | 更新工作空间 |
| DELETE | `/workspaces/{workspace_id}` | 是 | 删除工作空间 |
| GET | `/workspaces/{workspace_id}/permissions` | 是 | 权限列表 |
| POST | `/workspaces/{workspace_id}/permissions` | 是 | 授予权限 |
| DELETE | `/workspaces/{workspace_id}/permissions/{permission_id}` | 是 | 撤销权限 |
| GET | `/workspaces/{workspace_id}/documents` | 是 | 文档列表 |
| GET | `/workspaces/{workspace_id}/tools` | 是 | 工具列表 |
| GET | `/workspaces/{workspace_id}/stats` | 是 | 空间统计 |

`GET /workspaces` 查询参数：

- `page`（可选，默认 1）
- `page_size`（可选，默认 20）

### 6.6 工作流 Workflow

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/workflows` | 是 | 工作流列表 |
| POST | `/workflows` | 是 | 创建工作流 |
| GET | `/workflows/{workflow_id}` | 是 | 工作流详情 |
| DELETE | `/workflows/{workflow_id}` | 是 | 删除工作流 |
| POST | `/workflows/{workflow_id}/execute` | 是 | 触发执行（会创建任务并尝试分配） |
| GET | `/workflows/{workflow_id}/executions` | 是 | 执行记录列表 |
| GET | `/workflows/{workflow_id}/executions/{execution_id}` | 是 | 执行记录详情 |

`GET /workflows` 查询参数：

- `workspace_id`（可选）

`GET /workflows/{workflow_id}/executions` 查询参数：

- `limit`（可选）
- `offset`（可选）

### 6.7 消息 Message

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/messages` | 是 | 发送消息（agent/task/user/system） |
| GET | `/messages/user` | 是 | 当前用户消息列表 |
| GET | `/messages/user/unread-count` | 是 | 当前用户未读数量 |
| GET | `/messages/agent/{agent_id}` | 是 | 智能体消息列表 |
| GET | `/messages/task/{task_id}` | 是 | 任务消息列表 |
| POST | `/messages/{message_type}/{message_id}/read` | 是 | 标记已读 |
| DELETE | `/messages/{message_type}/{message_id}` | 是 | 删除消息 |
| POST | `/messages/{message_type}/read-batch` | 是 | 批量标记已读 |
| POST | `/messages/{message_type}/delete-batch` | 是 | 批量删除 |
| POST | `/messages/broadcast` | 是 | 发送系统广播 |

消息列表查询参数：

- `limit`（可选）
- `offset`（可选）

### 6.8 实时日志 Realtime Logs

| 方法 | 路径 | 鉴权 | 说明 |
|---|---|---|---|
| GET | `/logs/sse` | 是 | SSE实时日志流 |
| GET | `/logs/websocket` | 是 | WebSocket实时日志 |
| GET | `/logs/stats` | 是 | 实时日志统计 |

SSE/WebSocket日志过滤器参数：

- `level`（可选）：日志级别过滤，如 `["info", "warn"]`
- `target`（可选）：日志目标模块过滤
- `workspace_id`（可选）：工作空间ID过滤
- `task_id`（可选）：任务ID过滤
- `agent_id`（可选）：智能体ID过滤
- `user_id`（可选）：用户ID过滤

## 7. 关键请求示例

### 7.1 注册与登录

```bash
curl -X POST http://127.0.0.1:8000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice_01",
    "email": "alice@example.com",
    "password": "ChangeMe#123"
  }'
```

```bash
curl -X POST http://127.0.0.1:8000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice_01","password":"ChangeMe#123"}'
```

### 7.2 刷新令牌

`refresh_token` 同时支持蛇形和驼峰字段：`refresh_token` / `refreshToken`。

```bash
curl -X POST http://127.0.0.1:8000/api/v1/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token":"<refresh-token>"}'
```

### 7.3 创建任务（自动分配）

```bash
curl -X POST http://127.0.0.1:8000/api/v1/tasks \
  -H "Authorization: Bearer <access_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "分析Q1销售数据",
    "description": "生成趋势结论与改进建议",
    "priority": "medium",
    "workspace_id": "<workspace_uuid>",
    "requirements": {"capabilities": ["data_analysis"]},
    "context": {"dataset": "s3://bucket/q1.csv"},
    "metadata": {"source": "manual"}
  }'
```

### 7.4 智能体回调任务完成

```bash
curl -X POST http://127.0.0.1:8000/api/v1/agents/<agent_id>/complete-task \
  -H "Authorization: Bearer <access_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "task_id": "<task_uuid>",
    "success": true,
    "result": {"summary": "done", "score": 100}
  }'
```

### 7.5 创建并触发工作流

```bash
curl -X POST http://127.0.0.1:8000/api/v1/workflows \
  -H "Authorization: Bearer <access_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "销售分析工作流",
    "workspace_id": "<workspace_uuid>",
    "definition": {
      "nodes": [{"id":"collect"},{"id":"analyze"}],
      "edges": [["collect","analyze"]]
    }
  }'
```

```bash
curl -X POST http://127.0.0.1:8000/api/v1/workflows/<workflow_id>/execute \
  -H "Authorization: Bearer <access_token>" \
  -H "Content-Type: application/json" \
  -d '{"input":{"source":"api"},"options":{"priority":"high"}}'
```

## 8. 自动化测试工具

项目内置接口冒烟测试工具：

- 脚本：`scripts/api_test_tool.py`
- 默认覆盖：health、auth、workspace、workflow、task automation、message

运行示例：

```bash
python3 scripts/api_test_tool.py --base-url http://127.0.0.1:8000/api/v1 --timeout 15
```

## 9. 接口状态

所有接口均已实现，可通过以下方式验证：

- 访问 `/ui/endpoints` 查看完整接口列表
- 访问 `/ui/spec` 获取机器可读的API规范
- 使用内置测试工具进行功能验证
