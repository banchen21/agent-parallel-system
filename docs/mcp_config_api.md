# MCP 工具配置 API

本文档描述当前仓库里已经实现的 MCP 自建工具管理接口和工具定义格式。

## 数据结构

工具定义以 `McpToolDefinition` 为准。

核心字段：

1. `toolId`
2. `description`
3. `parameters`
4. `options`
5. `execution`

## 文件落盘位置

工具定义会被保存到：

```text
.mcps/<tool_id>.json
```

## API 列表

### 1. 获取全部工具

- 方法：`GET`
- 路径：`/api/v1/mcp/tools`

### 2. 创建工具

- 方法：`POST`
- 路径：`/api/v1/mcp/tools`

### 3. 更新工具

- 方法：`PUT`
- 路径：`/api/v1/mcp/tools/{tool_id}`

### 4. 删除工具

- 方法：`DELETE`
- 路径：`/api/v1/mcp/tools/{tool_id}`

## execution 字段说明

当前实现支持两种 transport：

1. `http`
2. `builtin`

### builtin

表示走系统内置工具分发器。

当前自动生成工具默认会使用：

```json
{
  "transport": "builtin",
  "endpoint": "terminal_run",
  "method": "POST"
}
```

## 自动生成工具的默认参数

当前自动生成工具使用的参数模型是：

1. `command`: 必填，shell 命令
2. `cwd`: 可选，执行目录
3. `timeout_ms`: 可选，执行超时

## 当前行为总结

1. 工具会同时保存到磁盘和内存
2. 自动生成工具已经可以直接执行
3. `builtin + terminal_run` 是当前最实用的自建工具执行方式
