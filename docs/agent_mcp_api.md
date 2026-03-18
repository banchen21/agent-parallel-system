# Agent 与 MCP API

本文档描述当前项目里与智能体管理、模型配置、MCP 工具使用相关的实际接口与行为。

## 范围

当前实现将“Agent 管理”和“MCP 工具管理”拆成两组接口：

1. Agent 生命周期与模型配置接口
2. MCP 自建工具 CRUD 接口

这两组接口都位于受保护作用域 `/api/v1` 下，需要 Bearer Access Token。

## Agent 接口

### 1. 获取 Agent 列表

- 方法：`GET`
- 路径：`/api/v1/agent`
- 认证：需要
- 行为：按当前登录用户过滤，只返回该用户工作区下的 Agent

返回字段以 `workspace::model::AgentInfo` 为准：

- `id`: UUID
- `name`: Agent 名称
- `kind`: `general | code | research | custom`
- `provider`: 使用的模型提供商名称
- `model`: 模型名
- `workspace_name`: 所属工作区
- `owner_username`: 所属用户
- `status`: `starting | running | stopping | stopped | unknown`
- `mcp_list`: 当前 Agent 绑定的工具 ID 列表

### 2. 创建 Agent

- 方法：`POST`
- 路径：`/api/v1/agent`
- 认证：需要

请求体：

```json
{
  "user_name": "banchen",
  "name": "executor-banchen",
  "kind": "general",
  "provider": "default",
  "model": "",
  "workspace_name": "banchen_default",
  "mcp_list": []
}
```

说明：

1. `kind` 支持 `general`、`code`、`research`、`custom`。
2. 创建时会写入数据库。
3. 当前实现还会在 `.workspaces/<workspace_name>/agents/<agent_name>/` 下创建对应目录。

### 3. 启动 Agent

- 方法：`POST`
- 路径：`/api/v1/agent/{agent_id}/start`
- 认证：需要

行为：

1. 启动或复用对应 `AgentActor`
2. 如果该 Agent 已绑定任务，启动阶段会尝试恢复 `accepted` 或 `executing` 状态的任务

### 4. 停止 Agent

- 方法：`POST`
- 路径：`/api/v1/agent/{agent_id}/stop`
- 认证：需要

行为：停止内存中的 Actor，不删除数据库记录。

### 5. 删除 Agent

- 方法：`DELETE`
- 路径：`/api/v1/agent/{agent_id}`
- 认证：需要

行为：

1. 校验当前用户权限
2. 删除 Agent 数据
3. 释放其运行态

## Provider 配置接口

### 1. 查询可选 Provider

- 方法：`GET`
- 路径：`/api/v1/agent/provider-options`
- 认证：需要

返回内容来自 `config/default.toml` 和当前 `Settings`。

### 2. 保存 Provider 配置

- 方法：`POST`
- 路径：`/api/v1/agent/provider-options`
- 认证：需要

请求体：

```json
{
  "provider": "deepseek",
  "model": "deepseek-chat",
  "token": "sk-xxx",
  "base_url": "https://api.silra.cn/v1"
}
```

行为：

1. 修改 `config/default.toml`
2. 更新 `[llm].default_provider`
3. 更新或新增 `[[providers]]` 块
4. 返回“配置已写入，重启后生效”

注意：当前实现是直接改配置文件，不是热更新运行中 Actor。

## MCP 自建工具接口

### 1. 获取工具列表

- 方法：`GET`
- 路径：`/api/v1/mcp/tools`
- 认证：需要

返回值为 `Vec<McpToolDefinition>`。

### 2. 创建工具

- 方法：`POST`
- 路径：`/api/v1/mcp/tools`
- 认证：需要

请求体示例：

```json
{
  "toolId": "shell_ls",
  "description": "列出目录文件",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "要执行的 shell 命令"
      }
    },
    "required": ["command"]
  },
  "options": {
    "timeoutMs": 30000,
    "maxRetries": 1
  },
  "execution": {
    "transport": "builtin",
    "endpoint": "terminal_run",
    "method": "POST",
    "headers": {}
  }
}
```

### 3. 更新工具

- 方法：`PUT`
- 路径：`/api/v1/mcp/tools/{tool_id}`
- 认证：需要

行为：

1. 路径参数会覆盖请求体中的 `toolId`
2. 工具定义会写入 `.mcps/<tool_id>.json`
3. 同步更新内存中的工具注册表

### 4. 删除工具

- 方法：`DELETE`
- 路径：`/api/v1/mcp/tools/{tool_id}`
- 认证：需要

行为：

1. 删除 `.mcps/<tool_id>.json`
2. 从内存工具列表移除

## Agent 与 MCP 的运行关系

当前代码中的关系如下：

1. Agent 发现自己有 `accepted` 或 `executing` 任务
2. Agent 调用 `McpAgentActor.ExecuteMcp`
3. MCP Actor 选择工具、补参数、执行工具、解释结果
4. 返回结构化结果给 Agent
5. Agent 根据 `success` 和 `should_retry` 推进任务状态

重点：

1. 直接执行失败且不可重试时，任务会直接落到 `completed_failure`
2. 执行成功时，任务进入 `submitted`，后续再进入审阅链路
3. 自动生成工具现在默认具备可执行配置，不再只是定义文件
