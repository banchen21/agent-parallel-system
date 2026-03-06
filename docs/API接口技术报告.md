# Agent Parallel System API 接口技术报告

## 1. 概览
*   **基础 URL**: `http://localhost:8000`
*   **技术栈**: Rust + Axum + SQLx
*   **认证方式**: JWT (Bearer Token)。所有受保护的接口需在 Header 中携带 `Authorization: Bearer <token>`。
*   **响应格式**: 统一使用 `application/json`。

---

## 2. 认证模块 (Auth)
用于用户生命周期管理及权限验证。

| 方法 | 端点 | 功能描述 | 主要参数 (JSON Body) |
| :--- | :--- | :--- | :--- |
| **POST** | `/auth/register` | 用户注册 | `username`, `email`, `password` |
| **POST** | `/auth/login` | 用户登录 | `email`, `password` |
| **POST** | `/auth/refresh` | 刷新令牌 | `refresh_token` |
| **GET** | `/auth/me` | 获取当前用户信息 | 无 (依赖 Token) |
| **POST** | `/auth/logout` | 退出登录 | 无 |

**核心 DTO 示例 (注册):**
```json
{
  "username": "agent_master",
  "email": "user@example.com",
  "password": "hashed_password_here"
}
```

---

## 3. 工作空间模块 (Workspace)
管理智能体运行的逻辑隔离环境。

| 方法 | 端点 | 功能描述 |
| :--- | :--- | :--- |
| **POST** | `/workspaces` | 创建新工作空间 |
| **GET** | `/workspaces` | 分页获取所属工作空间列表 |
| **GET** | `/workspaces/{id}` | 查看空间详情及配置 |
| **PUT** | `/workspaces/{id}` | 更新空间元数据或设置 |
| **DELETE** | `/workspaces/{id}` | 物理/逻辑删除工作空间 |
| **GET** | `/workspaces/{id}/stats` | 获取该空间内 Agent 和 Task 的运行统计信息 |

---

## 4. 任务调度模块 (Task)
系统核心。支持复杂依赖编排（DAG）。

| 方法 | 端点 | 功能描述 | 主要参数 |
| :--- | :--- | :--- | :--- |
| **POST** | `/tasks` | 创建并下发任务 | `title`, `task_type`, `input_data`, `priority` |
| **GET** | `/tasks` | 任务列表（支持按状态、空间筛选） | `workspace_id`, `status` |
| **GET** | `/tasks/{id}` | 获取任务详情及执行进度 | - |
| **PUT** | `/tasks/{id}/status` | 手动更新任务状态（管理端使用） | `status` (pending/running/done) |
| **POST** | `/tasks/{id}/decompose` | 调用 LLM 将大任务分解为子任务 | - |
| **GET** | `/tasks/{id}/subtasks` | 查询该任务衍生的所有子任务 | - |

**创建任务请求体示例:**
```json
{
  "workspace_id": "uuid-string",
  "title": "分析市场数据",
  "task_type": "data_analysis",
  "priority": "high",
  "input_data": { "target": "NASDAQ", "period": "2024" },
  "max_retries": 3
}
```

---

## 5. 智能体模块 (Agent)
管理 Agent 的注册、心跳及任务分配。

| 方法 | 端点 | 功能描述 |
| :--- | :--- | :--- |
| **GET** | `/agents` | 查看所有在线/忙碌的 Agent 列表 |
| **POST** | `/agents` | 注册一个新的 Agent 实例到系统 |
| **POST** | `/agents/{id}/heartbeat` | Agent 上报心跳，更新 `last_heartbeat_at` |
| **PUT** | `/agents/{id}/status` | 切换 Agent 状态（如切换为 `offline` 或 `idle`） |
| **POST** | `/agents/{id}/complete-task` | Agent 提交任务结果并领取下一个任务 |

---

## 6. 实时日志与消息 (Logging & Message)
基于 WebSocket 和 SSE 实现。

| 方法 | 端点 | 协议 | 功能描述 |
| :--- | :--- | :--- | :--- |
| **GET** | `/logs/sse` | SSE | 订阅实时系统日志流，支持按 `task_id` 过滤 |
| **GET** | `/logs/ws` | WS | 建立全双工实时通信，用于接收 Agent 状态变更消息 |
| **POST** | `/messages` | HTTP | 发送站内信或系统通知 |
| **GET** | `/messages/user` | HTTP | 获取用户的未读消息列表 |

**日志流过滤参数:**
*   `level`: info/warn/error
*   `task_id`: 过滤特定任务的执行日志

---

## 7. 错误代码参考 (Error Codes)

| 代码 | 含义 | 处理建议 |
| :--- | :--- | :--- |
| `200/201` | 成功 | - |
| `401` | Unauthorized | 检查 JWT Token 是否过期或缺失 |
| `403` | Forbidden | 当前用户不属于该工作空间 |
| `404` | Not Found | 任务或 Agent ID 不存在 |
| `429` | Rate Limit | Agent 请求心跳过于频繁 |
| `500` | Internal Error | 数据库连接失败或异步运行时异常 |

---

## 8. 总结与接口建议
1.  **原子性**: 任务分解接口 (`/decompose`) 建议异步化，并返回一个 `task_group_id` 供查询。
2.  **安全约束**: 鉴于 Agent 会频繁调用心跳接口，建议针对 `/agents/{id}/heartbeat` 采用轻量级的 API Key 验证而非复杂的 JWT 解析以提升性能。
3.  **数据一致性**: 目前数据库架构中 `tasks` 表和 `agent_assignments` 存在冗余，建议在 `PUT /tasks/{id}/status` 时通过事务同步更新两张表的数据。

---

## 相关文档
- [数据库架构设计](../migrations/database-schema.md)
- [OpenAPI 规范](./openapi.yaml)
- [技术规范文档](../technical-specification.md)
