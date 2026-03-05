# 基于LLM的多智能体并行协作系统

一个使用Rust构建的高性能、可扩展的多智能体并行协作系统，支持LLM集成、任务编排和实时通信。

## 功能特性

- 🚀 **高性能架构**: 使用Rust和异步编程构建
- 🤖 **多智能体协作**: 支持多个智能体并行处理任务
- 🧠 **LLM集成**: 集成OpenAI等大语言模型
- 🔄 **任务编排**: 智能任务分配和依赖管理
- 📊 **实时监控**: 完整的监控和日志系统
- 🔒 **安全认证**: JWT认证和权限管理
- 🐳 **容器化部署**: 支持Docker和Docker Compose
- 📈 **水平扩展**: 支持多实例部署和负载均衡

## 技术栈

- **后端**: Rust + Axum
- **数据库**: PostgreSQL
- **缓存/消息队列**: Redis
- **认证**: JWT
- **部署**: Docker + Docker Compose
- **监控**: Prometheus + Grafana

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
- API文档: http://localhost:8000/api/v1/docs
- 健康检查: http://localhost:8000/health

### 手动启动

如果您想手动启动服务：

```bash
# 启动数据库和Redis
docker-compose up -d postgres redis

# 运行数据库迁移
docker-compose run --rm api ./agent-parallel-system migrate

# 启动API服务
docker-compose up -d api

# 启动后台工作器
docker-compose up -d worker
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
├── Dockerfile             # Docker构建文件
├── docker-compose.yml     # Docker Compose配置
└── Cargo.toml            # Rust依赖配置
```

## API接口

### 认证接口
- `POST /api/v1/auth/register` - 用户注册
- `POST /api/v1/auth/login` - 用户登录
- `POST /api/v1/auth/refresh` - 刷新令牌
- `POST /api/v1/auth/logout` - 用户登出

### 任务接口
- `GET /api/v1/tasks` - 获取任务列表
- `POST /api/v1/tasks` - 创建新任务
- `GET /api/v1/tasks/{id}` - 获取任务详情
- `PUT /api/v1/tasks/{id}` - 更新任务
- `DELETE /api/v1/tasks/{id}` - 删除任务

### 智能体接口
- `GET /api/v1/agents` - 获取智能体列表
- `POST /api/v1/agents` - 创建智能体
- `GET /api/v1/agents/{id}` - 获取智能体详情
- `PUT /api/v1/agents/{id}` - 更新智能体
- `DELETE /api/v1/agents/{id}` - 删除智能体

### 工作空间接口
- `GET /api/v1/workspaces` - 获取工作空间列表
- `POST /api/v1/workspaces` - 创建工作空间
- `GET /api/v1/workspaces/{id}` - 获取工作空间详情
- `PUT /api/v1/workspaces/{id}` - 更新工作空间
- `DELETE /api/v1/workspaces/{id}` - 删除工作空间

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
docker-compose up -d postgres redis

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

## 部署

### 生产环境部署

1. 配置生产环境变量
```bash
cp .env.example .env.production
# 编辑生产环境配置
```

2. 构建和部署
```bash
# 使用Docker Compose
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d

# 或者使用Kubernetes
kubectl apply -f k8s/
```

### 监控和日志

系统集成了完整的监控和日志功能：

- **Prometheus**: 指标收集 (http://localhost:9090)
- **Grafana**: 仪表板 (http://localhost:3000)
- **结构化日志**: JSON格式日志输出

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