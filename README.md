# 基于LLM的多智能体并行协作系统

一个使用Rust构建的高性能、可扩展的多智能体并行协作系统，支持LLM集成、任务编排和实时通信。

> **项目状态**: 开发中 (完成度约 55-65%)
>
> 当前处于从"核心算法原型"向"完整工具软件"转化的关键阶段，已实现DAG编排、错误恢复和实时日志推送

## 项目完成度概览

| 模块 | 完成度 | 状态描述 |
|------|--------|----------|
| 核心后端逻辑 | 95% | 并行架构、任务分发、状态管理、DAG编排、错误恢复、实时日志已实现 |
| 接口适配层 | 40% | API骨架已建立，实时通信(SSE/WebSocket)已实现 |
| 前端界面 | 15% | 静态原型完成，数据绑定待实现 |
| 数据持久化 | 5% | 内存模式运行，数据库集成待开发 |

## 功能特性

### 已实现功能
- 🚀 **高性能架构**: 使用Rust和异步编程构建
- 🤖 **多智能体协作**: 支持多个智能体并行处理任务
- 🔄 **任务编排**: 智能任务分配和依赖管理
- 🧩 **DAG编排**: 复杂任务依赖关系的有向无环图编排
- 🔧 **错误恢复**: 自动重试、检查点和状态回滚机制
- 📊 **实时日志**: SSE/WebSocket实时日志推送和监控
- 🔒 **安全认证**: JWT认证和权限管理
- 🐳 **容器化部署**: 支持Docker和Docker Compose
- 🔄 **实时通信**: WebSocket/SSE实时日志推送

### 规划中功能
- 🧠 **LLM集成**: 集成OpenAI等大语言模型
- 📈 **高级监控**: 性能指标收集和可视化
- 🗄️ **数据持久化**: 完整的数据库集成

## 技术栈

- **后端**: Rust + Axum
- **数据库**: PostgreSQL 15 (规划中)
- **缓存/消息队列**: Redis 7 (已集成)
- **认证**: JWT + Argon2/Bcrypt (已实现)
- **部署**: Docker + Docker Compose
- **监控**: 结构化日志 + 实时日志推送 + 健康检查
- **实时通信**: SSE + WebSocket

## 快速开始

### 环境要求

- Docker 20.10+
- Docker Compose 2.0+

### 启动系统

1. 克隆项目
```bash
git clone <repository-url>
cd agent-parallel-system
```

2. 配置环境变量
```bash
cp .env.example .env
# 编辑.env文件配置您的环境变量
```

3. 启动服务
```bash
./scripts/start.sh
```

4. 访问系统
- API服务: http://localhost:8000
- API文档: http://localhost:8000/docs
- 健康检查: http://localhost:8000/health

### 手动启动

如果您想手动启动服务：

```bash
# 启动数据库和Redis
docker compose up -d postgres redis

# 运行数据库迁移
docker compose run --rm api ./agent-parallel-system migrate

# 启动API服务
docker compose up -d api

# 启动后台工作器
docker compose up -d worker
```

## 项目结构

```
agent-parallel-system/
├── src/                    # Rust源代码
│   ├── core/              # 核心模块（配置、数据库、错误处理）
│   ├── models/            # 数据模型
│   ├── services/          # 业务逻辑服务
│   ├── api/               # API路由
│   ├── middleware/        # 中间件
│   ├── utils/             # 工具函数
│   └── workers/           # 后台工作器
├── migrations/            # 数据库迁移文件
├── config/                # 配置文件
├── scripts/               # 部署脚本
├── tests/                 # 测试文件
├── storage/               # 文件存储
├── logs/                  # 日志文件
├── Dockerfile             # Docker构建文件
├── docker-compose.yml     # Docker Compose配置
└── Cargo.toml            # Rust依赖配置
```

## API接口 (开发中)

### API文档

- **完整API规范**: [api-specification.md](api-specification.md)
- **Web控制台**: http://localhost:8000/docs
- **机器可读接口**: http://localhost:8000/ui/spec
- **接口列表**: http://localhost:8000/ui/endpoints

### 接口总览 (规划中)

#### 系统与文档
- `GET /health` - 健康检查
- `GET /ready` - 就绪检查
- `GET /ui/spec` - API规范JSON
- `GET /ui/endpoints` - 接口列表JSON

#### 认证接口
- `POST /auth/register` - 用户注册
- `POST /auth/login` - 用户登录
- `POST /auth/refresh` - 刷新令牌
- `POST /auth/logout` - 用户登出
- `GET /auth/me` - 获取当前用户
- `POST /auth/change-password` - 修改密码

#### 任务接口
- `POST /tasks` - 创建任务（自动分配）
- `GET /tasks` - 获取任务列表
- `GET /tasks/{task_id}` - 获取任务详情
- `PUT /tasks/{task_id}` - 更新任务
- `DELETE /tasks/{task_id}` - 删除任务
- `PUT /tasks/{task_id}/status` - 更新任务状态
- `POST /tasks/{task_id}/decompose` - 任务分解
- `GET /tasks/{task_id}/subtasks` - 查询子任务

#### 智能体接口
- `GET /agents` - 获取智能体列表
- `POST /agents` - 注册智能体
- `GET /agents/{agent_id}` - 获取智能体详情
- `POST /agents/{agent_id}/heartbeat` - 上报心跳
- `PUT /agents/{agent_id}/status` - 更新状态
- `POST /agents/{agent_id}/assign-task` - 指定任务分配
- `POST /agents/{agent_id}/complete-task` - 上报任务完成
- `GET /agents/stats` - 获取智能体统计

#### 工作空间接口
- `POST /workspaces` - 创建工作空间
- `GET /workspaces` - 获取工作空间列表
- `GET /workspaces/{workspace_id}` - 获取工作空间详情
- `PUT /workspaces/{workspace_id}` - 更新工作空间
- `DELETE /workspaces/{workspace_id}` - 删除工作空间
- `GET /workspaces/{workspace_id}/permissions` - 权限列表
- `POST /workspaces/{workspace_id}/permissions` - 授予权限
- `DELETE /workspaces/{workspace_id}/permissions/{permission_id}` - 撤销权限
- `GET /workspaces/{workspace_id}/documents` - 文档列表
- `GET /workspaces/{workspace_id}/tools` - 工具列表
- `GET /workspaces/{workspace_id}/stats` - 空间统计

#### 工作流接口
- `GET /workflows` - 工作流列表
- `POST /workflows` - 创建工作流
- `GET /workflows/{workflow_id}` - 工作流详情
- `DELETE /workflows/{workflow_id}` - 删除工作流
- `POST /workflows/{workflow_id}/execute` - 触发执行
- `GET /workflows/{workflow_id}/executions` - 执行记录列表
- `GET /workflows/{workflow_id}/executions/{execution_id}` - 执行记录详情

#### 消息接口
- `POST /messages` - 发送消息
- `GET /messages/user` - 当前用户消息列表
- `GET /messages/user/unread-count` - 当前用户未读数量
- `GET /messages/agent/{agent_id}` - 智能体消息列表
- `GET /messages/task/{task_id}` - 任务消息列表
- `POST /messages/{message_type}/{message_id}/read` - 标记已读
- `DELETE /messages/{message_type}/{message_id}` - 删除消息
- `POST /messages/{message_type}/read-batch` - 批量标记已读
- `POST /messages/{message_type}/delete-batch` - 批量删除
- `POST /messages/broadcast` - 发送系统广播

## 开发指南

### 本地开发环境

1. 安装Rust工具链
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. 安装PostgreSQL和Redis
```bash
# Ubuntu/Debian
sudo apt-get install postgresql redis-server

# macOS
brew install postgresql redis
```

3. 运行应用
```bash
# 启动数据库和Redis
docker compose up -d postgres redis

# 运行数据库迁移
cargo run -- migrate

# 启动开发服务器
cargo run -- server

# 启动后台工作器
cargo run -- worker
```

### 测试

```bash
# 运行单元测试
cargo test

# 运行集成测试
cargo test --test integration

# 运行所有测试
cargo test --all
```

### 构建

```bash
# 调试构建
cargo build

# 发布构建
cargo build --release

# 构建Docker镜像
docker build -t agent-parallel-system .
```

## 开发路线图

### 已完成功能
- [x] 基础并行架构和任务分发
- [x] 智能体状态管理和负载均衡
- [x] 复杂任务依赖编排 (DAG)
- [x] 错误恢复和状态回滚机制
- [x] 任务检查点和自动重试

### 高优先级
- [ ] 打通前后端数据绑定
- [ ] 实现实时日志推送 (WebSocket)
- [ ] 完善任务创建功能

### 中优先级
- [ ] 实现身份校验和权限控制
- [ ] 优化DAG编排性能
- [ ] 添加工作流级别的错误恢复

### 低优先级
- [ ] 数据库集成 (PostgreSQL/Redis)
- [ ] 完善部署工具
- [ ] 性能优化和监控

## 配置说明

### 环境变量

主要环境变量配置：

- `DATABASE_URL`: PostgreSQL数据库连接字符串
- `REDIS_URL`: Redis连接字符串
- `JWT_SECRET`: JWT密钥
- `APP_ENV`: 应用环境 (development/production)
- `HOST`: 服务绑定地址
- `PORT`: 服务端口

### 数据库迁移

系统使用SQLx进行数据库迁移：

```bash
# 运行迁移
cargo run -- migrate

# 创建新迁移
cargo run -- migrate create <migration_name>
```

## 贡献指南

1. Fork项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建Pull Request

## 许可证

本项目采用MIT许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 联系方式

- 项目主页: [GitHub Repository]
- 问题反馈: [GitHub Issues]
- 邮箱: your.email@example.com

## 更新日志

### v0.1.0 (2024-01-01)
- 初始版本发布
- 基础多智能体架构
- RESTful API接口
- 容器化部署支持