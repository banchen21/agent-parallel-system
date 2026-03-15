# MCP 配置文件目录

此目录用于存储系统中所有可用的 MCP（Model Context Protocol）配置文件。

## 文件格式

每个 MCP 配置文件应为 JSON 格式，文件名为 `{mcp_name}.json`。

### 配置文件示例

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

## 管理

- 通过 API 接口添加/删除 MCP 配置
- 配置文件会自动同步到 `.mcps/mcp.json`
