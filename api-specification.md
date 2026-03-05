# Agent Parallel System API 文档（可运行版）

更新时间：2026-03-05

## 基础信息

- Base URL: `http://127.0.0.1:8000`
- 文档控制台: `/` 或 `/docs`
- 结构化规范: `/ui/spec`
- 健康检查: `/health`、`/ready`
- 认证: `Authorization: Bearer <access_token>`

## 统一响应格式

成功：
```json
{
  "success": true,
  "message": "操作成功",
  "data": {},
  "timestamp": "2026-03-05T14:00:00Z"
}
```

失败：
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "参数错误"
  },
  "timestamp": "2026-03-05T14:00:00Z"
}
```

## 实现状态

- 已实现：`Auth`、`Task`、`Agent`、`Workspace`、`UI/Docs`、`Health`
- 未实现：`Workflow`（返回 `501 Not Implemented`）

## Auth API

- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/refresh`
- `POST /auth/logout`
- `GET /auth/me`
- `POST /auth/change-password`

示例：
```bash
curl -X POST http://127.0.0.1:8000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","email":"alice@example.com","password":"Abcd1234"}'
```

```bash
curl -X POST http://127.0.0.1:8000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"Abcd1234"}'
```

## Task API

- `POST /tasks`
- `GET /tasks?workspace_id=<uuid>&status=<pending|in_progress|completed|failed|cancelled>&priority=<low|medium|high|urgent>&page=1&page_size=20`
- `GET /tasks/{task_id}`
- `PUT /tasks/{task_id}`
- `DELETE /tasks/{task_id}`
- `PUT /tasks/{task_id}/status`
- `POST /tasks/{task_id}/decompose`
- `GET /tasks/{task_id}/subtasks`

创建任务示例：
```json
{
  "title": "demo task",
  "description": "run",
  "priority": "medium",
  "workspace_id": "00000000-0000-0000-0000-000000000001",
  "requirements": {},
  "context": {},
  "metadata": {}
}
```

## Agent API

- `GET /agents?capabilities=analysis,report`
- `POST /agents`
- `GET /agents/{agent_id}`
- `POST /agents/{agent_id}/heartbeat`
- `PUT /agents/{agent_id}/status`
- `POST /agents/{agent_id}/assign-task`
- `POST /agents/{agent_id}/complete-task`
- `GET /agents/stats`

注册智能体示例：
```json
{
  "name": "analysis-agent-1",
  "description": "数据分析智能体",
  "capabilities": [
    {
      "name": "analysis",
      "description": "data analysis capability",
      "version": "1.0",
      "parameters": {}
    }
  ],
  "endpoints": {
    "task_execution": "http://127.0.0.1:9001/run",
    "health_check": "http://127.0.0.1:9001/health",
    "status_update": null
  },
  "limits": {
    "max_concurrent_tasks": 4,
    "max_execution_time": 600,
    "max_memory_usage": null,
    "rate_limit_per_minute": 60
  },
  "metadata": {}
}
```

## Workspace API

- `POST /workspaces`
- `GET /workspaces?page=1&page_size=20`
- `GET /workspaces/{workspace_id}`
- `PUT /workspaces/{workspace_id}`
- `DELETE /workspaces/{workspace_id}`
- `GET /workspaces/{workspace_id}/permissions`
- `POST /workspaces/{workspace_id}/permissions`
- `DELETE /workspaces/{workspace_id}/permissions/{permission_id}`
- `GET /workspaces/{workspace_id}/documents`
- `GET /workspaces/{workspace_id}/tools`
- `GET /workspaces/{workspace_id}/stats`

## Workflow API（未实现）

- `GET /workflows`
- `POST /workflows`
- `GET /workflows/{workflow_id}`
- `DELETE /workflows/{workflow_id}`
- `POST /workflows/{workflow_id}/execute`
- `GET /workflows/{workflow_id}/executions/{execution_id}`

当前返回 `501 Not Implemented`。
