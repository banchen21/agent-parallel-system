# Agent Parallel System

<div align="center">

**基于 Rust Actix 的高性能多智能体并行协作系统**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

[English](README_EN.md) | 简体中文

</div>

---

## 📖 项目简介

Agent Parallel System 是一个使用 Rust Actix Actor 框架构建的高性能、可扩展的多智能体并行协作系统。系统基于 Actor 模型实现智能体间的异步通信和状态管理，支持复杂任务的智能分解、多智能体协同执行、实时通信和完整的生命周期管理，适用于需要多个 AI 智能体协作完成复杂任务的场景。

### 核心特性

- 🚀 **高性能架构** - 基于 Rust + Actix Actor 模型，支持高并发异步处理
- 🎭 **Actor 模型** - 基于消息传递的并发模型，实现智能体间的松耦合通信
- 🤖 **多智能体协作** - 支持多个智能体并行处理任务，智能负载均衡
- 🧠 **智能记忆管理** - 基于 Neo4j 图数据库的长期记忆和短期记忆管理
- 📊 **实时系统监控** - 实时 CPU、内存、磁盘、网络监控和统计
- 🔄 **任务分类与路由** - 智能任务分类和意图分析，自动路由到合适的处理模块
- 🔐 **安全认证** - JWT 认证和授权，保护 API 接口安全
- 💬 **多渠道集成** - 支持 WebSocket、REST API 等多种通信渠道
- 🧩 **模块化设计** - 可插拔的智能体模块，易于扩展和定制
- 🐳 **容器化部署** - Docker + Docker Compose 一键部署

---

## 🏗️ 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     客户端层 (Client Layer)                   │
├─────────────────────────────────────────────────────────────┤
│  Web UI │  REST API │  WebSocket │  SSE 推送                 │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   API 网关层 (API Gateway)                   │
├─────────────────────────────────────────────────────────────┤
│  Actix Web 服务器 │  JWT 认证 │  请求路由 │  中间件处理        │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   Actor 系统层 (Actor System)                │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  聊天代理    │  任务代理     │  系统监控     │  用户管理      │
│  内存代理    │  通道管理     │  OpenAI代理  │  数据库代理     │
└──────────────┴──────────────┴──────────────┴───────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                 通信层 (Communication Layer)                 │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  Actor 消息  │  Redis Pub/Sub│  WebSocket  │  HTTP 调用     │
└──────────────┴──────────────┴──────────────┴───────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                   基础设施层 (Infrastructure)                │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  PostgreSQL  │  Redis       │  Neo4j       │  OpenAI API    │
└──────────────┴──────────────┴──────────────┴───────────────┘
```

### 核心模块

#### 1. 核心层 (Core)
- **Actor 系统** ([`actor_system.rs`](src/core/actor_system.rs)) - 系统监控和资源统计 Actor
- **配置管理** ([`config.rs`](src/core/config.rs)) - 统一配置加载和管理
- **请求处理** ([`handler.rs`](src/core/handler.rs)) - 系统监控 API 处理器
- **模块管理** ([`mod.rs`](src/core/mod.rs)) - 核心模块导出

#### 2. Actor 层 (Actors)
- **聊天代理** ([`chat_agent.rs`](src/chat/chat_agent.rs)) - 处理聊天消息和智能回复
- **任务代理** ([`task_agent.rs`](src/task_handler/task_agent.rs)) - 任务分类和意图分析
- **系统监控** ([`actor_system.rs`](src/core/actor_system.rs)) - 实时系统资源监控
- **用户管理** ([`actor_user.rs`](src/api/user/actor_user.rs)) - 用户认证和会话管理
- **内存代理** ([`actor_memory.rs`](src/graph_memory/actor_memory.rs)) - 基于 Neo4j 的智能记忆管理
- **通道管理** ([`actor_messages.rs`](src/chat/actor_messages.rs)) - 消息通道和会话管理
- **OpenAI 代理** ([`openai_actor.rs`](src/chat/openai_actor.rs)) - OpenAI API 调用封装
- **数据库代理** ([`actor_database.rs`](src/channel/actor_database.rs)) - PostgreSQL 数据库操作
- **Redis 代理** ([`redis_actor.rs`](src/api/redis_actor.rs)) - Redis 缓存和 Pub/Sub
- **任务处理** ([`actor_task.rs`](src/task_handler/actor_task.rs)) - 任务执行和管理

#### 3. API 层 (API)
- **认证模块** ([`auth.rs`](src/api/auth.rs)) - JWT 认证和授权中间件
- **用户接口** ([`handler.rs`](src/api/user/handler.rs)) - 用户相关 API 端点
- **聊天接口** ([`handler.rs`](src/chat/handler.rs)) - 聊天消息 API 端点
- **系统监控接口** ([`handler.rs`](src/core/handler.rs)) - 系统资源监控 API

#### 4. 数据模型 (Models)
- **用户模型** ([`model.rs`](src/api/user/model.rs)) - 用户数据结构和数据库模型
- **聊天模型** ([`model.rs`](src/chat/model.rs)) - 聊天消息和会话模型
- **任务模型** ([`task_model.rs`](src/task_handler/task_model.rs)) - 任务分类和响应模型

#### 5. 工具层 (Utils)
- **环境变量工具** ([`env_util.rs`](src/utils/env_util.rs)) - 环境变量读取和默认值处理
- **JSON 工具** ([`json_util.rs`](src/utils/json_util.rs)) - JSON 数据处理和清理

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
- **PostgreSQL** 15+ (用于用户数据和消息存储)
- **Redis** 7+ (用于缓存和会话管理)
- **Neo4j** 5.0+ (用于智能记忆图数据库，可选)
- **OpenAI API Key** (或兼容的 OpenAI API 服务)

### 方式一：本地开发部署

1. **克隆项目**
```bash
git clone https://github.com/banchen21/agent-parallel-system.git
cd agent-parallel-system
```

2. **安装 Rust 和依赖**
```bash
# 安装 Rust (如果尚未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 安装数据库服务
# Ubuntu/Debian
sudo apt-get install postgresql redis-server

# macOS
brew install postgresql redis neo4j
```

3. **配置环境变量**
```bash
# 复制环境变量模板
cp .env.example .env

# 编辑 .env 文件，配置必要的环境变量
# 主要配置项：
# DATABASE_URL=postgres://postgres:password@localhost:5432/agent_system
# REDIS_URL=redis://localhost:6379
# NEO4J_URI=bolt://localhost:7687
# NEO4J_USERNAME=neo4j
# NEO4J_PASSWORD=neo4j
# OPENAI_API_KEY=your_openai_api_key
# OPENAI_BASE_URL=https://api.openai.com/v1
# JWT_SECRET=your_jwt_secret_key
```

4. **初始化数据库**
```bash
# 创建 PostgreSQL 数据库
createdb agent_system

# 运行数据库迁移
cargo install sqlx-cli --no-default-features --features postgres
sqlx migrate run

# 启动 Redis
redis-server --daemonize yes

# 启动 Neo4j (如果使用)
neo4j start
```

5. **编译和运行**
```bash
# 开发模式运行
cargo run

# 或者生产模式编译后运行
cargo build --release
./target/release/agent-parallel-system
```

6. **验证安装**
```bash
# 健康检查
curl http://localhost:8000/health

# 用户注册
curl -X POST http://localhost:8000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"testuser","password":"testpass"}'
```

### 方式二：Docker 部署

1. **使用 Docker Compose**
```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f
```

2. **单独使用 Docker**
```bash
# 构建镜像
docker build -t agent-parallel-system .

# 运行容器
docker run -d \
  -p 8000:8000 \
  --env-file .env \
  agent-parallel-system
```

### 访问系统

- **API 服务**: http://localhost:8000
- **健康检查**: http://localhost:8000/health
- **认证接口**: http://localhost:8000/auth/*
- **受保护 API**: http://localhost:8000/api/v1/*

### 开发工具

```bash
# 代码格式化
cargo fmt

# 代码检查
cargo clippy

# 运行测试
cargo test

# 生成文档
cargo doc --open
```

---

## 📚 API 文档

### 认证接口（无需认证）

#### 用户注册
```http
POST /auth/register
Content-Type: application/json

{
  "username": "user123",
  "password": "secure_password"
}
```

#### 用户登录
```http
POST /auth/login
Content-Type: application/json

{
  "username": "user123",
  "password": "secure_password"
}
```

#### 刷新令牌
```http
POST /auth/refresh
Content-Type: application/json

{
  "refresh_token": "your_refresh_token"
}
```

### 受保护接口（需要 Bearer Token）

#### 系统监控统计
```http
GET /api/v1/stats
Authorization: Bearer <access_token>
```

响应示例：
```json
{
  "cpu_usage": 15.5,
  "used_memory": 2048576000,
  "total_memory": 17179869184,
  "disk_usage": 536870912000,
  "total_disk": 1000204886016,
  "net_rx_speed": 1234.56,
  "net_tx_speed": 789.01
}
```

#### 发送聊天消息
```http
POST /api/v1/chat/message
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "channel_id": "general",
  "content": "你好，今天天气怎么样？",
  "user_id": "user_123"
}
```

#### 获取消息历史
```http
GET /api/v1/chat/history/{channel_id}
Authorization: Bearer <access_token>
```

### 健康检查接口

#### 系统健康状态
```http
GET /health
```

响应示例：
```json
{
  "status": "healthy",
  "timestamp": "2026-03-10T22:21:54.080Z"
}
```

完整的 API 文档请访问: http://localhost:8000/docs (待实现)

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

- **项目主页**: [\[Agent Parallel System\]](https://github.com/banchen21/agent-parallel-system)
- **问题反馈**: [\[GitHub Issues\]](https://github.com/banchen21/agent-parallel-system/issues)
- **邮箱**: banchen19@outlook.com

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
