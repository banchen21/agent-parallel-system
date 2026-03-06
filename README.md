# Agent Parallel System

<div align="center">

**基于 Rust 的高性能多智能体并行协作系统**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

[English](README_EN.md) | 简体中文

</div>

---

## 📖 项目简介

Agent Parallel System 是一个使用 Rust 构建的高性能、可扩展的多智能体并行协作系统。系统支持复杂任务的智能分解、多智能体协同执行、实时通信和完整的生命周期管理，适用于需要多个 AI 智能体协作完成复杂任务的场景。

### 核心特性

- 🚀 **高性能架构** - 基于 Rust + Tokio 异步运行时，支持高并发处理
- 🤖 **多智能体协作** - 支持多个智能体并行处理任务，智能负载均衡
- 🔄 **DAG 任务编排** - 有向无环图任务依赖管理，支持复杂工作流
- 🛡️ **容错与恢复** - 自动重试、检查点、状态回滚机制
- 📊 **实时监控** - SSE/WebSocket 实时日志推送和任务状态监控
- 🔐 **安全认证** - JWT + Argon2/Bcrypt 加密，RBAC 权限控制
- 💬 **多渠道集成** - 支持 Telegram、Discord、QQ、Web 等多种通信渠道
- 🧠 **LLM 集成** - 支持 OpenAI、Ollama 等多种大语言模型
- 🐳 **容器化部署** - Docker + Docker Compose 一键部署
- 📈 **可扩展设计** - 模块化架构，易于扩展和定制

---

## 🏗️ 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     客户端层 (Client Layer)                   │
├─────────────────────────────────────────────────────────────┤
│  Web UI │  REST API │  Telegram │  Discord │  WebSocket     │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   API 网关层 (API Gateway)                   │
├─────────────────────────────────────────────────────────────┤
│  认证授权 │  速率限制 │  请求路由 │  CORS 处理               │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   核心服务层 (Core Services)                 │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  任务服务    │  智能体服务   │  工作空间服务 │  编排器服务    │
│  工作流服务  │  消息服务     │  聊天服务     │  通道服务      │
└──────────────┴──────────────┴──────────────┴───────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                 通信层 (Communication Layer)                 │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  Redis 队列  │  WebSocket   │  SSE 推送    │  消息路由      │
└──────────────┴──────────────┴──────────────┴───────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   基础设施层 (Infrastructure)                │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  PostgreSQL  │  Redis       │  对象存储     │  LLM 服务      │
└──────────────┴──────────────┴──────────────┴───────────────┘
```

### 核心模块

#### 1. 核心层 (Core)
- **配置管理** ([`config.rs`](src/core/config.rs)) - 统一配置加载和管理
- **数据库** ([`database.rs`](src/core/database.rs)) - PostgreSQL 和 Redis 连接池
- **错误处理** ([`errors.rs`](src/core/errors.rs)) - 统一错误类型定义
- **DAG 编排** ([`dag.rs`](src/core/dag.rs)) - 任务依赖关系管理
- **错误恢复** ([`error_recovery.rs`](src/core/error_recovery.rs)) - 自动重试和恢复机制
- **实时日志** ([`realtime_logging.rs`](src/core/realtime_logging.rs)) - SSE/WebSocket 日志推送
- **安全模块** ([`security.rs`](src/core/security.rs)) - JWT 和密码加密

#### 2. 服务层 (Services)
- **认证服务** ([`auth_service.rs`](src/services/auth_service.rs)) - 用户注册、登录、JWT 管理
- **任务服务** ([`task_service.rs`](src/services/task_service.rs)) - 任务 CRUD 和生命周期管理
- **智能体服务** ([`agent_service.rs`](src/services/agent_service.rs)) - 智能体注册、健康检查、负载均衡
- **工作空间服务** ([`workspace_service.rs`](src/services/workspace_service.rs)) - 工作空间和权限管理
- **编排器服务** ([`orchestrator_service.rs`](src/services/orchestrator_service.rs)) - 任务分配和智能体协调
- **工作流服务** ([`workflow_service.rs`](src/services/workflow_service.rs)) - 工作流定义和执行
- **消息服务** ([`message_service.rs`](src/services/message_service.rs)) - 智能体间消息传递
- **聊天服务** ([`chat_service.rs`](src/services/chat_service.rs)) - 聊天会话和消息管理
- **通道服务** ([`channel_service.rs`](src/services/channel_service.rs)) - 多渠道集成管理
- **LLM 客户端** ([`llm_client.rs`](src/services/llm_client.rs)) - 大语言模型调用封装
- **图数据库** ([`graph_db.rs`](src/services/graph_db.rs)) - 知识图谱存储
- **记忆服务** ([`memory_service.rs`](src/services/memory_service.rs)) - 上下文记忆管理

#### 3. 数据模型 (Models)
- **用户模型** ([`user.rs`](src/models/user.rs)) - 用户、会话、权限
- **任务模型** ([`task.rs`](src/models/task.rs)) - 任务定义、状态、依赖
- **智能体模型** ([`agent.rs`](src/models/agent.rs)) - 智能体配置、能力、状态
- **工作空间模型** ([`workspace.rs`](src/models/workspace.rs)) - 工作空间、成员、权限
- **消息模型** ([`message.rs`](src/models/message.rs)) - 各类消息类型
- **工作流模型** ([`workflow.rs`](src/models/workflow.rs)) - 工作流定义和执行
- **通道模型** ([`channel.rs`](src/models/channel.rs)) - 通道配置和用户映射
- **聊天模型** ([`chat.rs`](src/models/chat.rs)) - 聊天会话和消息

#### 4. API 层 (API)
- **路由定义** ([`routes.rs`](src/api/routes.rs)) - RESTful API 端点
- **Swagger 文档** ([`swagger.rs`](src/api/swagger.rs)) - OpenAPI 规范
- **Web UI** ([`web_ui.html`](src/api/web_ui.html)) - 简单的 Web 界面

#### 5. 后台工作器 (Workers)
- **任务工作器** ([`task_worker.rs`](src/workers/task_worker.rs)) - 后台任务处理
- **清理工作器** ([`cleanup_worker.rs`](src/workers/cleanup_worker.rs)) - 定期数据清理
- **通知工作器** ([`notification_worker.rs`](src/workers/notification_worker.rs)) - 异步通知发送

---

## 🗄️ 数据库架构

系统使用 PostgreSQL 作为主数据库，包含以下核心表：

### 核心表结构

- **users** - 用户信息和认证
- **user_sessions** - 用户会话管理
- **workspaces** - 工作空间定义
- **workspace_members** - 工作空间成员
- **workspace_permissions** - 细粒度权限
- **tasks** - 任务定义和状态
- **task_dependencies** - 任务依赖关系
- **agents** - 智能体注册信息
- **agent_assignments** - 智能体任务分配
- **messages** - 消息记录
- **workflows** - 工作流定义
- **workflow_executions** - 工作流执行记录
- **channel_configs** - 通道配置
- **channel_users** - 通道用户映射
- **chat_sessions** - 聊天会话
- **chat_messages** - 聊天消息
- **llm_configs** - LLM 配置

详细的数据库架构文档请参考 [`migrations/database-schema.md`](migrations/database-schema.md)

---

## 🚀 快速开始

### 环境要求

- **Rust** 1.70+
- **PostgreSQL** 15+
- **Redis** 7+
- **Docker** 20.10+ (可选)
- **Docker Compose** 2.0+ (可选)

### 方式一：Docker Compose 部署（推荐）

1. **克隆项目**
```bash
git clone <repository-url>
cd agent-parallel-system
```

2. **配置环境变量**
```bash
cp .env.example .env
# 编辑 .env 文件，配置数据库和 Redis 连接信息
```

3. **启动服务**
```bash
./scripts/start.sh
```

4. **访问系统**
- API 服务: http://localhost:8000
- API 文档: http://localhost:8000/docs
- 健康检查: http://localhost:8000/health

### 方式二：本地开发部署

1. **安装依赖**
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 PostgreSQL 和 Redis
# Ubuntu/Debian
sudo apt-get install postgresql redis-server

# macOS
brew install postgresql redis
```

2. **配置数据库**
```bash
# 创建数据库
createdb agent_system

# 设置环境变量
export DATABASE_URL="postgres://postgres:password@localhost:5432/agent_system"
export REDIS_URL="redis://localhost:6379"
```

3. **运行数据库迁移**
```bash
# 安装 sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# 运行迁移
sqlx migrate run

# 生成查询缓存（用于编译时 SQL 验证）
cargo sqlx prepare
```

4. **编译和运行**
```bash
# 开发模式
cargo run

# 生产模式
cargo build --release
./target/release/agent-parallel-system
```

---

## 📚 API 文档

### 认证接口

#### 用户注册
```http
POST /api/v1/auth/register
Content-Type: application/json

{
  "username": "user123",
  "email": "user@example.com",
  "password": "secure_password"
}
```

#### 用户登录
```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "user123",
  "password": "secure_password"
}
```

### 任务接口

#### 创建任务
```http
POST /api/v1/tasks
Authorization: Bearer <token>
Content-Type: application/json

{
  "workspace_id": "uuid",
  "title": "任务标题",
  "description": "任务描述",
  "priority": "high",
  "requirements": {}
}
```

#### 查询任务
```http
GET /api/v1/tasks/{task_id}
Authorization: Bearer <token>
```

### 智能体接口

#### 注册智能体
```http
POST /api/v1/agents
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "智能体名称",
  "agent_type": "llm",
  "capabilities": ["coding", "analysis"],
  "workspace_id": "uuid"
}
```

### 工作流接口

#### 创建工作流
```http
POST /api/v1/workflows
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "工作流名称",
  "workspace_id": "uuid",
  "definition": {
    "steps": [...]
  }
}
```

#### 执行工作流
```http
POST /api/v1/workflows/{workflow_id}/execute
Authorization: Bearer <token>
Content-Type: application/json

{
  "input": {},
  "options": {}
}
```

完整的 API 文档请访问: http://localhost:8000/docs

---

## 🔧 配置说明

系统配置文件位于 [`config/default.toml`](config/default.toml)，主要配置项：

### 服务器配置
```toml
[server]
host = "0.0.0.0"
port = 8000
workers = 4
api_prefix = "/api/v1"
```

### 数据库配置
```toml
[database]
url = "postgres://postgres:password@localhost:5432/agent_system"
max_connections = 20
min_connections = 5
```

### Redis 配置
```toml
[redis]
url = "redis://localhost:6379"
pool_size = 10
```

### JWT 配置
```toml
[jwt]
secret = "your-secret-key"
access_token_expire_minutes = 30
refresh_token_expire_days = 7
```

### LLM 配置
```toml
[openai]
api_key = "your-openai-api-key"
model = "gpt-4"
max_tokens = 4096
temperature = 0.7
```

---

## 🧪 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name

# 运行集成测试
cargo test --test integration_tests

# 生成测试覆盖率报告
cargo tarpaulin --out Html
```

---

## 📊 性能优化

### 编译优化

生产环境编译配置（[`Cargo.toml`](Cargo.toml:101)）：
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = 'abort'
```

### 数据库优化

- 使用连接池复用数据库连接
- 为高频查询字段创建索引
- 使用 `sqlx::query!` 宏进行编译时 SQL 验证
- 批量操作使用事务

### 缓存策略

- Redis 缓存热点数据
- 会话状态存储在 Redis
- 使用 Redis Pub/Sub 实现实时通信

---

## 🐳 Docker 部署

### 构建镜像
```bash
docker build -t agent-parallel-system .
```

### 运行容器
```bash
docker run -d \
  -p 8000:8000 \
  -e DATABASE_URL="postgres://..." \
  -e REDIS_URL="redis://..." \
  agent-parallel-system
```

### Docker Compose
```bash
docker-compose up -d
```

---

## 📖 开发指南

### 项目结构
```
agent-parallel-system/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 库入口
│   ├── api/                 # API 层
│   ├── core/                # 核心功能
│   ├── models/              # 数据模型
│   ├── services/            # 业务服务
│   ├── middleware/          # 中间件
│   ├── utils/               # 工具函数
│   └── workers/             # 后台工作器
├── migrations/              # 数据库迁移
├── config/                  # 配置文件
├── scripts/                 # 部署脚本
├── docs/                    # 文档
└── tests/                   # 测试
```

### 添加新功能

1. 在 `src/models/` 定义数据模型
2. 在 `src/services/` 实现业务逻辑
3. 在 `src/api/routes.rs` 添加 API 端点
4. 在 `migrations/` 添加数据库迁移
5. 编写单元测试和集成测试

### 代码规范

```bash
# 格式化代码
cargo fmt

# 代码检查
cargo clippy

# 运行测试
cargo test
```

---

## 🔍 故障排查

### SQLx 编译错误

**错误**: `set DATABASE_URL to use query macros online`

**解决方案**:
```bash
# 方案 1: 设置环境变量
export DATABASE_URL="postgres://postgres:password@localhost:5432/agent_system"

# 方案 2: 生成离线查询缓存
cargo sqlx prepare
```

### 数据库连接失败

检查：
1. PostgreSQL 服务是否运行
2. 数据库 URL 配置是否正确
3. 防火墙是否允许连接
4. 数据库用户权限是否足够

### Redis 连接失败

检查：
1. Redis 服务是否运行
2. Redis URL 配置是否正确
3. Redis 是否需要密码认证

---

## 📈 监控和日志

### 日志级别

通过环境变量设置：
```bash
export RUST_LOG=info  # debug, info, warn, error
```

### 实时日志

系统支持通过 SSE 或 WebSocket 实时推送日志：
```http
GET /api/v1/logs/stream?task_id=<uuid>
```

### 健康检查

```http
GET /health
```

返回：
```json
{
  "status": "healthy",
  "database": "connected",
  "redis": "connected",
  "version": "0.1.0"
}
```

---

## 🤝 贡献指南

我们欢迎所有形式的贡献！

1. Fork 本项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

### 贡献规范

- 遵循 Rust 代码规范
- 添加必要的测试
- 更新相关文档
- 提交信息清晰明确

---

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## 🔗 相关资源

- [架构设计文档](architecture.md)
- [技术规格文档](technical-specification.md)
- [数据库架构文档](migrations/database-schema.md)
- [API 接口技术报告](docs/API接口技术报告.md)
- [Rust 官方文档](https://www.rust-lang.org/)
- [Axum 框架文档](https://docs.rs/axum/)
- [SQLx 文档](https://docs.rs/sqlx/)

---

## 📞 联系方式

- **项目主页**: [GitHub Repository]
- **问题反馈**: [GitHub Issues]
- **邮箱**: your.email@example.com

---

## 🎯 路线图

### v0.2.0 (计划中)
- [ ] 完整的数据库持久化
- [ ] 高级工作流编排
- [ ] 性能监控和指标收集
- [ ] 分布式追踪

### v0.3.0 (计划中)
- [ ] 多租户支持
- [ ] 智能体市场
- [ ] 可视化工作流编辑器
- [ ] 高级安全特性

### v1.0.0 (计划中)
- [ ] 生产级稳定性
- [ ] 完整的文档和示例
- [ ] 性能优化
- [ ] 企业级功能

---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给我们一个 Star！⭐**

Made with ❤️ by Agent Parallel System Team

</div>
