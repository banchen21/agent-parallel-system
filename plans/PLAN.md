# APS 聊天和通道层 - 最终架构计划总结

## 执行摘要

本文档总结了为 APS 系统添加聊天和通道层的完整架构设计。该设计支持多个外部通道（Telegram、Discord、QQ、Web），通过统一的消息路由系统连接到聊天层，最终集成 OpenAI 兼容的 LLM 服务和原有的 APS 系统。

### 关键决策

| 决策项 | 选择 | 理由 |
|--------|------|------|
| LLM 支持 | OpenAI + 本地模型 | 灵活性强，支持离线和云端 |
| 通道优先级 | Telegram + Web Chat | 最常用，易于测试和部署 |
| 历史存储 | 完整数据库存储 | 便于上下文管理和分析 |
| APS 集成 | 通过现有 API 调用 | 最小化改动，易于维护 |

---

## 架构层次

### 第 1 层：外部通道层 (Channel Layer)

**组件：**
- `TelegramAdapter` - Telegram Bot 集成
- `WebAdapter` - Web Chat WebSocket 集成
- `DiscordAdapter` - Discord Bot 集成（第二阶段）
- `QQAdapter` - QQ Bot 集成（第二阶段）

**职责：**
- 接收来自各平台的消息
- 将平台特定格式转换为统一格式
- 处理平台特定的业务逻辑
- 发送响应回原平台

**接口：**
```rust
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    fn channel_type(&self) -> ChannelType;
    async fn send_message(&self, channel_user_id: &str, content: &str, metadata: Option<Value>) -> Result<String>;
    async fn handle_webhook(&self, payload: Value) -> Result<ChannelMessage>;
    async fn validate_config(&self, config: &Value) -> Result<()>;
    async fn get_user_info(&self, channel_user_id: &str) -> Result<Value>;
}
```

### 第 2 层：消息路由层 (Message Router Layer)

**组件：**
- `MessageRouterService` - 消息路由和转发

**职责：**
- 验证和标准化消息
- 识别或创建用户身份
- 查找或创建聊天会话
- 路由消息到聊天层
- 处理响应并回复到原通道

**核心流程：**
```
Input Message
    ↓
Validate & Normalize
    ↓
Find/Create ChannelUser
    ↓
Find/Create ChatSession
    ↓
Route to ChatService
    ↓
Get Response
    ↓
Send to ChannelAdapter
    ↓
Output Message
```

### 第 3 层：聊天层 (Chat Layer)

**组件：**
- `ChatService` - 聊天逻辑处理
- `SessionService` - 会话管理
- `LLMClient` - LLM 集成

**职责：**
- 获取会话历史
- 构建 LLM 提示词
- 调用 LLM API
- 处理流式响应
- 保存消息历史
- 管理上下文窗口

**支持的 LLM：**
- OpenAI (gpt-3.5-turbo, gpt-4, etc.)
- 本地模型 (Ollama, LM Studio)
- 可扩展支持其他提供商

### 第 4 层：APS 集成层 (APS Integration Layer)

**职责：**
- 通过现有 API 调用 APS 系统
- 获取任务、智能体、工作空间信息
- 执行工作流和任务
- 集成编排器功能

**集成点：**
- 任务服务 API
- 智能体服务 API
- 工作空间服务 API
- 编排器服务 API

---

## 数据库设计

### 新增表结构

```sql
-- 1. 通道配置表
channel_configs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50),
    name VARCHAR(255),
    config JSONB,
    enabled BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)

-- 2. 通道用户映射表
channel_users (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    channel_type VARCHAR(50),
    channel_user_id VARCHAR(255),
    channel_username VARCHAR(255),
    metadata JSONB,
    created_at TIMESTAMP
)

-- 3. 聊天会话表
chat_sessions (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    channel_type VARCHAR(50),
    title VARCHAR(255),
    model VARCHAR(100),
    status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    last_message_at TIMESTAMP
)

-- 4. 聊天消息表
chat_messages (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES chat_sessions(id),
    role VARCHAR(50),
    content TEXT,
    tokens_used INTEGER,
    metadata JSONB,
    created_at TIMESTAMP
)

-- 5. 通道消息日志表
channel_message_logs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50),
    channel_message_id VARCHAR(255),
    user_id UUID REFERENCES users(id),
    direction VARCHAR(50),
    content TEXT,
    status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP
)

-- 6. LLM 配置表
llm_configs (
    id UUID PRIMARY KEY,
    provider VARCHAR(50),
    name VARCHAR(255),
    api_key_encrypted VARCHAR(500),
    base_url VARCHAR(500),
    model_name VARCHAR(100),
    config JSONB,
    enabled BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)
```

---

## 项目结构

```
src/
├── channels/                    # 通道层
│   ├── mod.rs
│   ├── adapter.rs              # 通道适配器接口
│   ├── telegram.rs             # Telegram 适配器
│   ├── web.rs                  # Web Chat 适配器
│   ├── discord.rs              # Discord 适配器（第二阶段）
│   └── qq.rs                   # QQ 适配器（第二阶段）
│
├── chat/                        # 聊天层
│   ├── mod.rs
│   ├── service.rs              # ChatService
│   ├── session.rs              # SessionService
│   ├── llm_client.rs           # LLM 客户端
│   └── router.rs               # MessageRouterService
│
├── models/
│   ├── channel.rs              # 通道相关模型
│   ├── chat.rs                 # 聊天相关模型
│   └── ...（现有模型）
│
├── services/
│   ├── chat_service.rs         # 聊天服务
│   ├── session_service.rs      # 会话服务
│   ├── message_router_service.rs # 消息路由服务
│   └── ...（现有服务）
│
├── api/
│   ├── channels.rs             # 通道管理 API
│   ├── chat.rs                 # 聊天 API
│   ├── webhooks.rs             # Webhook 处理
│   └── ...（现有 API）
│
└── ...（现有结构）
```

---

## API 端点设计

### 通道管理 API

```
POST   /api/channels                    创建通道配置
GET    /api/channels                    列出所有通道
GET    /api/channels/:id                获取通道详情
PUT    /api/channels/:id                更新通道配置
DELETE /api/channels/:id                删除通道配置
POST   /api/channels/:id/test           测试通道连接
```

### 聊天 API

```
POST   /api/chat/sessions               创建聊天会话
GET    /api/chat/sessions               列出用户会话
GET    /api/chat/sessions/:id           获取会话详情
PUT    /api/chat/sessions/:id           更新会话
DELETE /api/chat/sessions/:id           删除会话

POST   /api/chat/messages               发送消息
GET    /api/chat/sessions/:id/messages  获取会话消息
DELETE /api/chat/messages/:id           删除消息
```

### Webhook 端点

```
POST   /webhooks/telegram               Telegram Webhook
POST   /webhooks/discord                Discord Webhook
POST   /webhooks/qq                     QQ Webhook
```

### WebSocket 端点

```
WS     /ws/chat                         Web Chat WebSocket
```

---

## 实现阶段

### 第一阶段：核心基础设施（优先级：高）
- [ ] 创建数据库迁移脚本
- [ ] 定义数据模型
- [ ] 实现通道适配器接口
- [ ] 实现 Telegram 适配器
- [ ] 实现 Web Chat 适配器

### 第二阶段：聊天层（优先级：高）
- [ ] 实现 LLM 客户端
- [ ] 实现 ChatService
- [ ] 实现 SessionService
- [ ] 实现 MessageRouterService
- [ ] 集成 OpenAI API

### 第三阶段：API 和集成（优先级：中）
- [ ] 创建通道管理 API
- [ ] 创建聊天 API
- [ ] 创建 Webhook 处理端点
- [ ] 实现 WebSocket 支持
- [ ] 集成 APS 系统 API

### 第四阶段：扩展和优化（优先级：中）
- [ ] 实现 Discord 适配器
- [ ] 实现 QQ 适配器
- [ ] 添加本地模型支持
- [ ] 性能优化
- [ ] 错误处理和重试机制

### 第五阶段：测试和部署（优先级：中）
- [ ] 单元测试
- [ ] 集成测试
- [ ] 前端聊天界面
- [ ] 文档更新
- [ ] 部署和监控

---

## 关键技术决策

### 1. 消息格式标准化

所有通道消息转换为统一格式：
```rust
pub struct ChannelMessage {
    pub id: String,
    pub channel_type: ChannelType,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Value>,
}
```

### 2. 会话管理

- 每个用户在每个通道有独立的会话
- 会话包含完整的对话历史
- 支持会话切换和多会话管理
- 自动清理过期会话

### 3. LLM 集成

- 支持多个 LLM 提供商
- 支持模型切换
- Token 计数和管理
- 流式响应处理
- 错误重试机制

### 4. 错误处理

- 通道级别错误处理
- 消息路由错误处理
- LLM 调用错误处理
- 数据库错误处理
- 用户友好的错误消息

---

## 配置示例

### Telegram 配置

```toml
[channels.telegram]
enabled = true
bot_token = "YOUR_BOT_TOKEN"
webhook_url = "https://your-domain.com/webhooks/telegram"
webhook_secret = "your-secret-key"
```

### OpenAI 配置

```toml
[llm.openai]
provider = "openai"
api_key = "sk-..."
model = "gpt-3.5-turbo"
temperature = 0.7
max_tokens = 2000
```

### 本地模型配置

```toml
[llm.ollama]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama2"
temperature = 0.7
```

---

## 安全考虑

1. **API 密钥管理**
   - 加密存储敏感信息
   - 使用环境变量
   - 定期轮换密钥

2. **消息验证**
   - 验证 Webhook 签名
   - 验证用户身份
   - 防止消息重放

3. **速率限制**
   - 按用户限制请求频率
   - 按通道限制请求频率
   - 防止 DDoS 攻击

4. **数据隐私**
   - 加密敏感数据
   - 定期清理日志
   - 遵守数据保护法规

---

## 监控和日志

### 关键指标

- 消息处理延迟
- LLM API 调用成功率
- 通道连接状态
- 会话活跃度
- Token 使用情况

### 日志级别

- ERROR: 错误和异常
- WARN: 警告和异常情况
- INFO: 重要事件
- DEBUG: 调试信息
- TRACE: 详细跟踪

---

## 下一步行动

1. **审查和批准** - 确认架构设计符合需求
2. **环境准备** - 准备开发环境和依赖
3. **实现第一阶段** - 开始核心基础设施开发
4. **迭代和优化** - 根据反馈进行调整
5. **部署和监控** - 上线和持续监控

---

## 参考文档

- [`chat-channel-architecture.md`](chat-channel-architecture.md) - 架构概览
- [`implementation-roadmap.md`](implementation-roadmap.md) - 实现路线图
- [`detailed-architecture.md`](detailed-architecture.md) - 详细设计
