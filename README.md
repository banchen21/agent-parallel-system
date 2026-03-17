# Agent Parallel System

<div align="center">

**基于 Rust + Actix Actor 的多智能体并行协作系统**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## 项目简介

Agent Parallel System 是一个以 Actor 模型为核心的后端系统，聚焦以下能力：

- 多智能体任务调度与执行（AgentManager + AgentActor）
- 任务编排与审阅决策（DagOrchestrActor + TaskAgent）
- 聊天与任务协同（ChatAgent + WebSocket）
- 记忆管理（Neo4j 图记忆）
- 实时日志流（SSE）
- JWT 认证与多工作区隔离

系统对外提供 REST + WebSocket + SSE 三类接口，前端可直接接入任务管理、聊天、日志控制台等页面。

---

## 当前架构

### 分层视图

1. 接入层
- HTTP API（Actix Web）
- WebSocket 聊天连接
- SSE 日志流

2. 业务层（Actor）
- ChatAgent：聊天推理与消息组织
- TaskAgent：任务识别、审阅与决策
- DagOrchestrActor：任务状态流转与通知
- AgentManagerActor / AgentActor：智能体生命周期、分配与执行
- ChannelManagerActor：消息持久化与广播
- AgentMemoryActor：图记忆读写

3. 基础设施层
- PostgreSQL：任务、消息、用户、工作区等核心数据
- Redis：缓存/会话相关能力
- Neo4j：长期记忆图谱
- OpenAI 兼容提供商：LLM 推理

### 关键流程（任务）

1. 用户提交任务后写入 tasks
2. DagOrchestrActor 分配可用 agent，任务进入 accepted/executing
3. AgentActor 执行 MCP，成功后任务进入 submitted
4. DagOrchestrActor 触发 TaskAgent 审阅，任务进入 under_review
5. 用户在前端接收/拒绝审阅结果
- 接收：completed_success 或 completed_failure
- 拒绝：回到 published 并重新分配

### submitted 卡住保护机制

已实现两层兜底：

1. 审阅消息投递失败缓存 + 重试队列
2. submitted 遗留任务定时回补扫描

相关参数可在配置中调整：

- task_review.submitted_recover_scan_interval_secs
- task_review.first_retry_delay_secs

---

## 代码结构（实际）

```text
src/
├── main.rs
├── agsnets/            # AgentActor、AgentManagerActor、agent API
├── api/                # auth、user、redis actor
├── channel/            # 通道消息持久化与广播
├── chat/               # ChatAgent、WS handler、history handler
├── core/               # config、系统监控、日志流 handler
├── graph_memory/       # Neo4j 记忆 actor
├── mcp/                # MCP actor 与配置
├── postgre_database/   # DB 初始化与迁移兼容
├── task/               # 任务编排、任务接口、任务审阅
├── utils/              # 工具函数（含日志广播层）
└── workspace/          # 工作区 actor 与 API
```

---

## API 总览（当前实现）

### 公开接口

- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/refresh`
- `GET /ws/chat?token=<access_token>`
- `GET /logs/stream?token=<access_token>`

### 受保护接口（`/api/v1` + Bearer Token）

- 系统
  - `GET /api/v1/system_info`

- 聊天
  - `GET /api/v1/message`（历史消息）

- 工作区
  - `GET /api/v1/workspace`
  - `POST /api/v1/workspace`
  - `DELETE /api/v1/workspace/{name}`

- 任务
  - `GET /api/v1/tasks`
  - `GET /api/v1/tasks/{task_id}`
  - `POST /api/v1/tasks`
  - `POST /api/v1/tasks/{task_id}/review-decision`
  - `DELETE /api/v1/tasks/{task_id}`（仅允许删除已完成/已取消任务）

- 智能体
  - `GET /api/v1/agent`
  - `POST /api/v1/agent`

---

## 配置说明

主配置文件：`config/default.toml`

重点配置段：

- `limits`：历史记录分页大小
- `agents.running_loop_interval_secs`：Agent 任务轮询周期
- `task_review.submitted_recover_scan_interval_secs`：submitted 回补扫描周期
- `task_review.first_retry_delay_secs`：首次重投延迟
- `features.enable_memory_query`：记忆查询开关
- `llm` + `[[providers]]`：多模型提供商配置
- `chat_agent` / `task_agent` / `memory_agent` / `mcp_agent`：提示词配置

---

## 本地启动

### 依赖

- Rust 1.70+
- PostgreSQL
- Redis
- Neo4j（可选但建议）

### 环境变量（示例）

```bash
DATABASE_URL=postgres://postgres:password@localhost:5432/agent_system
REDIS_URL=redis://127.0.0.1:6379
NEO4J_URI=127.0.0.1:7687
NEO4J_USERNAME=neo4j
NEO4J_PASSWORD=Neo4j123456
LOG_LEVEL=info
```

### 运行

```bash
cargo run
```

服务默认监听：`0.0.0.0:8000`

---

## 开发与排查

### 常用命令

```bash
cargo fmt
cargo check
cargo test
```

### 典型问题

1. 任务长时间停在 submitted
- 检查 TaskAgent 是否注册成功
- 检查 `task_review` 配置是否过大
- 查看日志流确认是否触发回补扫描

2. 前端任务列表数量不全
- 检查前端布局是否可滚动
- 检查当前用户工作区过滤条件

3. 聊天回复提前“宣布结果”
- 检查 `chat_agent.prompt` 的事实约束是否生效
- 重启服务使新配置生效

---

## 许可证

MIT
