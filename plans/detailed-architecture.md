# APS 聊天和通道层 - 完整架构设计文档

## 1. 系统架构总览

### 1.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        外部通道层 (Channel Layer)                     │
├──────────────────────────┬──────────────────────────────────────────┤
│  Telegram Bot API        │  Web Chat (WebSocket)                    │
│  - Webhook 接收          │  - 实时双向通信                           │
│  - 消息发送              │  - 浏览器客户端                           │
└──────────────────────────┴──────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
        ┌─────────────────────────┐    ┌─────────────────────────┐
        │  TelegramAdapter        │    │  WebAdapter             │
        │  - 消息标准化           │    │  - WebSocket 管理       │
        │  - 用户映射             │    │  - 会话管理             │
        │  - 回复处理             │    │  - 实时推送             │
        └─────────────────────────┘    └─────────────────────────┘
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
        ┌─────────────────────────────────────────────────────────┐
        │         消息路由层 (Message Router Layer)                │
        ├─────────────────────────────────────────────────────────┤
        │  • 消息标准化和验证                                      │
        │  • 用户身份识别和映射                                    │
        │  • 会话查找或创建                                        │
        │  • 消息路由到聊天层                                      │
        │  • 响应回复到原通道                                      │
        └─────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
        ┌─────────────────────────┐    ┌─────────────────────────┐
        │  ChatService            │    │  SessionService         │
        │  - LLM 调用             │    │  - 会话管理             │
        │  - 对话历史管理         │    │  - 上下文维护           │
        │  - 流式响应处理         │    │  - 消息持久化           │
        │  - 错误重试             │    │  - 会话清理             │
        └─────────────────────────┘    └─────────────────────────┘
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
        ┌─────────────────────────────────────────────────────────┐
        │         LLM 集成层 (LLM Integration Layer)               │
        ├─────────────────────────────────────────────────────────┤
        │  • OpenAI API 客户端                                     │
        │  • 本地模型支持 (Ollama/LM Studio)                       │
        │  • Token 计数和管理                                      │
        │  • 模型切换和负载均衡                                    │
        └─────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
        ┌─────────────────────────┐    ┌─────────────────────────┐
        │  OpenAI API             │    │  本地模型                │
        │  - gpt-3.5-turbo        │    │  - Ollama               │
        │  - gpt-4                │    │  - LM Studio            │
        │  - 其他模型             │    │  - 自定义模型           │
        └─────────────────────────┘    └─────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
        ┌─────────────────────────┐    ┌─────────────────────────┐
        │  APS 系统集成           │    │  数据库层               │
        │  - 任务服务调用         │    │  - PostgreSQL           │
        │  - 智能体服务调用       │    │  - 消息存储             │
        │  - 工作空间访问         │    │  - 会话管理             │
        │  - 编排器集成           │    │  - 配置存储             │
        └─────────────────────────┘    └─────────────────────────┘
```

### 1.2 消息流程图

```
用户在 Telegram 发送消息
        │
        ▼
TG Webhook → TelegramAdapter.handle_webhook()
        │
        ▼
MessageRouterService.route_message()
        │
        ├─ 验证消息
        ├─ 查找或创建 ChannelUser
        ├─ 查找或创建 ChatSession
        │
        ▼
ChatService.process_message()
        │
        ├─ 获取会话历史
        ├─ 构建 LLM 提示词
        ├─ 调用 LLM API
        │
        ▼
LLM 返回响应
        │
        ├─ 保存用户消息到数据库
        ├─ 保存助手响应到数据库
        ├─ 更新会话元数据
        │
        ▼
MessageRouterService.send_response()
        │
        ├─ 获取原通道适配器
        ├─ 格式化响应
        │
        ▼
TelegramAdapter.send_message()
        │
        ▼
用户在 Telegram 收到回复
```

---

## 2. 核心组件详细设计

### 2.1 通道适配器接口

```rust
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// 获取通道类型
    fn channel_type(&self) -> ChannelType;
    
    /// 发送消息到通道
    async fn send_message(
        &self,
        channel_user_id: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String, AppError>;
    
    /// 处理来自通道的 Webhook 回调
    async fn handle_webhook(&self, payload: Value) -> Result<ChannelMessage, AppError>;
    
    /// 验证通道配置
    async fn validate_config(&self, config: &Value) -> Result<(), AppError>;
    
    /// 获取用户信息
    async fn get_user_info(&self, channel_user_id: &str) -> Result<Value, AppError>;
}
```

### 2.2 Telegram 适配器实现

**关键特性：**
- 通过 Webhook 接收消息
- 支持文本、图片、文件等多种消息类型
- 处理 Telegram 特定的功能（inline keyboard、callback query 等）
- 自动重试机制

**配置示例：**
```json
{
  "bot_token": "YOUR_BOT_TOKEN",
  "webhook_url": "https://your-domain.com/webhooks/telegram",
  "webhook_secret": "your-secret-key"
}
```

### 2.3 Web Chat 适配器实现

**关键特性：**
- WebSocket 实时双向通信
- 会话管理
- 消息队列处理
- 自动重连机制

**连接流程：**
```
1. 客户端连接 WebSocket
2. 发送认证令牌
3. 创建或恢复会话
4. 接收消息
5. 发送消息
6. 接收响应
```

---

## 3. 服务层设计

### 3.1 ChatService

**职责：**
- 处理聊天逻辑
- 调用 LLM API
- 管理对话历史
- 处理流式响应

**核心方法：**
```rust
pub struct ChatService {
    db: PgPool,
    llm_client: LLMClient,
    session_service: Arc<SessionService>,
}

impl ChatService {
    /// 处理用户消息
    pub async fn process_message(
        &self,
        session_id: Uuid,
        user_message: &str,
    ) -> Result<String, AppError>;
    
    /// 获取会话历史
    pub async fn get_history(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatMessage>, AppError>;
    
    /// 清空会话历史
    pub async fn clear_history(&self, session_id: Uuid) -> Result<(), AppError>;
    
    /// 切换模型
    pub async fn switch_model(
        &self,
        session_id: Uuid,
        model: &str,
    ) -> Result<(), AppError>;
}
```

### 3.2 SessionService

**职责：**
- 创建和管理聊天会话
- 维护会话状态
- 处理会话清理

**核心方法：**
```rust
pub struct SessionService {
    db: PgPool,
}

impl SessionService {
    /// 创建新会话
    pub async fn create_session(
        &self,
        user_id: Uuid,
        channel_type: Option<&str>,
        title: Option<&str>,
    ) -> Result<ChatSession, AppError>;
    
    /// 获取或创建会话
    pub async fn get_or_create_session(
        &self,
        user_id: Uuid,
        channel_type: Option<&str>,
    ) -> Result<ChatSession, AppError>;
    
    /// 获取用户的所有会话
    pub async fn get_user_sessions(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ChatSession>, AppError>;
    
    /// 更新会话
    pub async fn update_session(
        &self,
        session_id: Uuid,
        title: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), AppError>;
    
    /// 删除会话
    pub async fn delete_session(&self, session_id: Uuid) -> Result<(), AppError>;
}
```

### 3.3 MessageRouterService

**职责：**
- 路由消息到聊天层
- 管理通道用户映射
- 处理响应回复

**核心方法：**
```rust
pub struct MessageRouterService {
    db: PgPool,
    chat_service: Arc<ChatService>,
    channel_adapters: HashMap<ChannelType, Arc<dyn ChannelAdapter>>,
}

impl MessageRouterService {
    /// 路由消息
    pub async fn route_message(
        &self,
        channel_msg: ChannelMessage,
    ) -> Result<(), AppError>;
    
    /// 发送响应到通道
    pub async fn send_response(
        &self,
        channel_type: ChannelType,
        channel_user_id: &str,
        content: &str,
    ) -> Result<(), AppError>;
    
    /// 获取或创建通道用户
    pub async fn get_or_create_channel_user(
        &self,
        user_id: Uuid,
        channel_type: ChannelType,
        channel_user_id: &str,
        channel_username: Option<&str>,
    ) -> Result<ChannelUser, AppError>;
}
```

### 3.4 LLMClient

**职责：**
- 调用 LLM API
- 管理多个 LLM 提供商
- 处理流式响应
- Token 计数

**支持的提供商：**
- OpenAI (gpt-3.5-turbo, gpt-4 等)
- 本地模型 (Ollama, LM Studio)

**核心方法：**
```rust
pub struct LLMClient {
    configs: HashMap<String, LLMConfig>,
    http_client: reqwest::Client,
}

impl LLMClient {
    /// 调用 LLM
    pub async fn chat_completion(
        &self,
        provider: &str,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, AppError>;
    
    /// 流式调用 LLM
    pub async fn chat_completion_stream(
        &self,
        provider: &str,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<impl Stream<Item = Result<String, AppError>>, AppError>;
    
    /// 计算 Token 数
    pub fn count_tokens(&self, text: &str, model: &str) -> usize;
}
```

---

## 4. API 端点设计

### 4.1 聊天 API

```
POST   /api/chat/sessions              - 创建聊天会话
GET    /api/chat/sessions              - 获取用户的所有会话
GET    /api/chat/sessions/:session_id  - 获取会话详情
PUT    /api/chat/sessions/:session_id  - 更新会话
DELETE /api/chat/sessions/:session_id  - 删除会话

POST   /api/chat/messages              - 发送消息
GET    /api/chat/messages/:session_id  - 获取会话消息历史
DELETE /api/chat/messages/:session_id  - 清空会话历史

POST   /api/chat/sessions/:session_id/model - 切换模型
```

### 4.2 通道管理 API

```
POST   /api/channels                   - 创建通道配置
GET    /api/channels                   - 列出所有通道
GET    /api/channels/:channel_id       - 获取通道详情
PUT    /api/channels/:channel_id       - 更新通道配置
DELETE /api/channels/:channel_id       - 删除通道

POST   /api/channels/:channel_id/test  - 测试通道连接
```

### 4.3 Webhook 端点

```
POST   /webhooks/telegram              - Telegram Webhook
POST   /webhooks/discord               - Discord Webhook (后续)
POST   /webhooks/qq                    - QQ Webhook (后续)
```

### 4.4 WebSocket 端点

```
WS     /ws/chat                        - Web Chat WebSocket
```

---

## 5. 数据库设计

### 5.1 表结构

```sql
-- 通道配置
CREATE TABLE channel_configs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    config JSONB NOT NULL,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- 通道用户映射
CREATE TABLE channel_users (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    channel_type VARCHAR(50) NOT NULL,
    channel_user_id VARCHAR(255) NOT NULL,
    channel_username VARCHAR(255),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

-- 聊天会话
CREATE TABLE chat_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    channel_type VARCHAR(50),
    title VARCHAR(255),
    model VARCHAR(100) DEFAULT 'gpt-3.5-turbo',
    status VARCHAR(50) DEFAULT 'active',
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    last_message_at TIMESTAMP
);

-- 聊天消息
CREATE TABLE chat_messages (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES chat_sessions(id),
    role VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    tokens_used INTEGER,
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

-- LLM 配置
CREATE TABLE llm_configs (
    id UUID PRIMARY KEY,
    provider VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    api_key_encrypted VARCHAR(500),
    base_url VARCHAR(500),
    model_name VARCHAR(100),
    config JSONB,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- 通道消息日志
CREATE TABLE channel_message_logs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50) NOT NULL,
    channel_message_id VARCHAR(255),
    user_id UUID REFERENCES users(id),
    direction VARCHAR(50),
    content TEXT,
    status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);
```

---

## 6. 配置管理

### 6.1 环境变量

```bash
# OpenAI 配置
OPENAI_API_KEY=sk-...
OPENAI_API_BASE=https://api.openai.com/v1
OPENAI_MODEL=gpt-3.5-turbo

# 本地模型配置
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama2

# Telegram 配置
TELEGRAM_BOT_TOKEN=your-bot-token
TELEGRAM_WEBHOOK_URL=https://your-domain.com/webhooks/telegram
TELEGRAM_WEBHOOK_SECRET=your-secret

# 聊天配置
CHAT_MAX_HISTORY=50
CHAT_SESSION_TIMEOUT=3600
CHAT_DEFAULT_MODEL=gpt-3.5-turbo
```

### 6.2 配置文件 (config/default.toml)

```toml
[chat]
max_history = 50
session_timeout = 3600
default_model = "gpt-3.5-turbo"
enable_streaming = true

[llm]
default_provider = "openai"
timeout_seconds = 30
retry_attempts = 3

[channels]
enabled = ["telegram", "web"]

[telegram]
webhook_path = "/webhooks/telegram"
message_timeout = 300

[web]
max_connections = 1000
message_queue_size = 100
```

---

## 7. 实现阶段

### 第一阶段：基础设施
- [ ] 创建数据库迁移脚本
- [ ] 定义数据模型
- [ ] 实现通道适配器接口

### 第二阶段：通道层
- [ ] 实现 Telegram 适配器
- [ ] 实现 Web Chat 适配器
- [ ] 实现通道管理服务

### 第三阶段：聊天层
- [ ] 实现 LLM 客户端
- [ ] 实现 ChatService
- [ ] 实现 SessionService

### 第四阶段：消息路由
- [ ] 实现 MessageRouterService
- [ ] 实现消息验证和标准化
- [ ] 实现错误处理和重试

### 第五阶段：API 和集成
- [ ] 创建聊天 API 端点
- [ ] 创建通道管理 API 端点
- [ ] 创建 Webhook 处理端点
- [ ] 实现 WebSocket 支持

### 第六阶段：测试和优化
- [ ] 单元测试
- [ ] 集成测试
- [ ] 性能测试
- [ ] 文档编写

---

## 8. 关键技术决策

### 8.1 为什么选择这个架构？

1. **分层设计** - 清晰的职责分离，易于维护和扩展
2. **适配器模式** - 支持多个通道，易于添加新通道
3. **异步处理** - 使用 Tokio 处理高并发
4. **消息队列** - 使用 Redis 处理消息缓冲
5. **完整历史** - 所有消息持久化，支持上下文维护

### 8.2 扩展性考虑

- **新通道添加** - 只需实现 ChannelAdapter trait
- **新 LLM 提供商** - 扩展 LLMClient 支持
- **性能优化** - 可添加消息缓存、向量数据库等
- **多租户支持** - 已在数据模型中考虑

---

## 9. 安全考虑

1. **API 密钥加密** - 所有敏感信息加密存储
2. **认证授权** - 使用现有的 JWT 认证
3. **消息验证** - Webhook 签名验证
4. **速率限制** - 防止滥用
5. **审计日志** - 记录所有操作

---

## 10. 监控和日志

1. **消息日志** - 所有消息记录到数据库
2. **错误追踪** - 详细的错误日志
3. **性能指标** - Token 使用、响应时间等
4. **健康检查** - 通道和 LLM 连接状态

