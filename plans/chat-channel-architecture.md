# APS 系统聊天和通道层架构设计

## 1. 系统架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                    外部通道层 (Channel Layer)                     │
├──────────────┬──────────────┬──────────────┬────────────────────┤
│  Telegram    │  Discord     │  QQ          │  Web Chat          │
│  (TG)        │  (DC)        │  (QQ)        │  (WebSocket)       │
└──────────────┴──────────────┴──────────────┴────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│              消息路由和适配层 (Message Router)                    │
├─────────────────────────────────────────────────────────────────┤
│  • 通道消息标准化                                                 │
│  • 用户身份映射                                                   │
│  • 会话管理                                                       │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│               聊天层 (Chat Layer)                                 │
├─────────────────────────────────────────────────────────────────┤
│  • OpenAI 兼容接口集成                                            │
│  • 对话历史管理                                                   │
│  • 上下文维护                                                     │
│  • 流式响应处理                                                   │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│              原 APS 系统 (Original APS)                           │
├──────────────┬──────────────┬──────────────┬────────────────────┤
│  任务服务    │  智能体服务   │  工作空间服务 │  编排器服务        │
└──────────────┴──────────────┴──────────────┴────────────────────┘
```

## 2. 核心组件设计

### 2.1 通道层 (Channel Layer)

**职责：**
- 接收来自各个外部平台的消息
- 将平台特定的消息格式转换为统一格式
- 管理通道连接和认证
- 处理平台特定的业务逻辑（如 TG 的 inline keyboard、DC 的 embed 等）

**支持的通道：**
1. **Telegram (TG)** - 通过 Bot API
2. **Discord (DC)** - 通过 Discord Bot
3. **QQ** - 通过 QQ Bot API
4. **Web Chat** - 通过 WebSocket

**通道接口定义：**
```rust
pub trait ChannelAdapter: Send + Sync {
    async fn send_message(&self, channel_msg: ChannelMessage) -> Result<String>;
    async fn receive_message(&self) -> Result<ChannelMessage>;
    async fn handle_callback(&self, payload: Value) -> Result<ChannelMessage>;
    fn channel_type(&self) -> ChannelType;
}
```

### 2.2 消息路由系统 (Message Router)

**职责：**
- 接收来自通道层的消息
- 标准化消息格式
- 管理用户会话和上下文
- 路由消息到聊天层
- 处理响应并回复到原通道

**核心流程：**
```
Channel Message → Normalize → Session Lookup → Chat Layer → Response → Channel Reply
```

### 2.3 聊天层 (Chat Layer)

**职责：**
- 对接 OpenAI 兼容接口（如 OpenAI、Ollama、LM Studio 等）
- 管理对话历史
- 维护用户上下文
- 处理流式响应
- 集成 APS 系统的智能体能力

**特性：**
- 支持多种 LLM 模型
- 对话历史持久化
- 上下文窗口管理
- 流式响应支持
- 错误重试机制

## 3. 数据模型

### 3.1 新增数据库表

```sql
-- 通道配置表
CREATE TABLE channel_configs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    config JSONB NOT NULL,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- 通道用户映射表
CREATE TABLE channel_users (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    channel_type VARCHAR(50) NOT NULL,
    channel_user_id VARCHAR(255) NOT NULL,
    channel_username VARCHAR(255),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(channel_type, channel_user_id)
);

-- 聊天会话表
CREATE TABLE chat_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    channel_type VARCHAR(50),
    title VARCHAR(255),
    model VARCHAR(100),
    status VARCHAR(50) DEFAULT 'active',
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    last_message_at TIMESTAMP
);

-- 聊天消息表
CREATE TABLE chat_messages (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES chat_sessions(id),
    role VARCHAR(50) NOT NULL, -- 'user', 'assistant', 'system'
    content TEXT NOT NULL,
    tokens_used INTEGER,
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

-- 通道消息日志表
CREATE TABLE channel_message_logs (
    id UUID PRIMARY KEY,
    channel_type VARCHAR(50) NOT NULL,
    channel_message_id VARCHAR(255),
    user_id UUID REFERENCES users(id),
    direction VARCHAR(50), -- 'inbound', 'outbound'
    content TEXT,
    status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);
```

### 3.2 Rust 数据模型

```rust
// 通道类型
pub enum ChannelType {
    Telegram,
    Discord,
    QQ,
    WebChat,
}

// 统一的通道消息格式
pub struct ChannelMessage {
    pub id: String,
    pub channel_type: ChannelType,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Value>,
}

// 聊天会话
pub struct ChatSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel_type: Option<ChannelType>,
    pub title: String,
    pub model: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// 聊天消息
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String, // 'user', 'assistant', 'system'
    pub content: String,
    pub tokens_used: Option<i32>,
    pub created_at: DateTime<Utc>,
}
```

## 4. 服务架构

### 4.1 新增服务

1. **ChannelService** - 管理通道配置和适配器
2. **ChatService** - 处理聊天逻辑和 LLM 集成
3. **MessageRouterService** - 路由和转发消息
4. **SessionService** - 管理聊天会话

### 4.2 服务交互流程

```
1. 用户在 TG 发送消息
   ↓
2. TG Webhook → ChannelAdapter (TG)
   ↓
3. MessageRouterService.route_message()
   ↓
4. 查找或创建 ChatSession
   ↓
5. ChatService.process_message()
   ↓
6. 调用 OpenAI API
   ↓
7. 保存消息历史
   ↓
8. 返回响应到 MessageRouterService
   ↓
9. ChannelAdapter (TG) 发送回复
   ↓
10. 用户在 TG 收到回复
```

## 5. API 端点设计

### 5.1 通道管理 API

```
POST   /api/channels                    - 创建通道配置
GET    /api/channels                    - 列出所有通道
GET    /api/channels/:channel_id        - 获取通道详情
PUT    /api/channels/:channel_id        - 更新通道配置
DELETE /api/channels/:channel_id        - 删除通道
POST   /api/channels/:channel_id/test   - 测试通道连接
```

### 5.2 聊天 API

```
POST   /api/chat/sessions               - 创建聊天会话
GET    /api/chat/sessions               - 列出用户的会话
GET    /api/chat/sessions/:session_id   - 获取会话详情
POST   /api/chat/sessions/:session_id/messages - 发送消息
GET    /api/chat/sessions/:session_id/messages - 获取消息历史
DELETE /api/chat/sessions/:session_id   - 删除会话
```

### 5.3 通道 Webhook API

```
POST   /api/webhooks/telegram           - TG Webhook
POST   /api/webhooks/discord            - Discord Webhook
POST   /api/webhooks/qq                 - QQ Webhook
```

## 6. 实现步骤

### Phase 1: 基础架构
- [ ] 创建通道层模块结构
- [ ] 定义通道接口和数据模型
- [ ] 创建数据库表
- [ ] 实现 ChannelService

### Phase 2: 聊天层
- [ ] 创建聊天层模块
- [ ] 实现 OpenAI 兼容接口集成
- [ ] 实现 ChatService
- [ ] 实现会话管理

### Phase 3: 消息路由
- [ ] 实现 MessageRouterService
- [ ] 实现消息标准化
- [ ] 实现用户映射

### Phase 4: 通道适配器
- [ ] 实现 Telegram 适配器
- [ ] 实现 Discord 适配器
- [ ] 实现 QQ 适配器
- [ ] 实现 Web Chat 适配器

### Phase 5: API 和集成
- [ ] 实现通道管理 API
- [ ] 实现聊天 API
- [ ] 实现 Webhook 端点
- [ ] 集成到现有 APS 系统

### Phase 6: 测试和优化
- [ ] 单元测试
- [ ] 集成测试
- [ ] 性能优化
- [ ] 文档编写

## 7. 配置示例

### 7.1 环境变量

```env
# OpenAI 配置
OPENAI_API_KEY=sk-...
OPENAI_API_BASE=https://api.openai.com/v1
OPENAI_MODEL=gpt-4

# Telegram 配置
TELEGRAM_BOT_TOKEN=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11
TELEGRAM_WEBHOOK_URL=https://your-domain.com/api/webhooks/telegram

# Discord 配置
DISCORD_BOT_TOKEN=your-discord-bot-token
DISCORD_WEBHOOK_URL=https://your-domain.com/api/webhooks/discord

# QQ 配置
QQ_BOT_ID=your-qq-bot-id
QQ_BOT_TOKEN=your-qq-bot-token
QQ_WEBHOOK_URL=https://your-domain.com/api/webhooks/qq
```

## 8. 关键设计决策

1. **消息标准化** - 所有通道消息转换为统一格式，便于处理
2. **会话隔离** - 每个用户在每个通道有独立的会话
3. **异步处理** - 使用 Tokio 异步处理消息，提高吞吐量
4. **缓存策略** - 使用 Redis 缓存会话和用户映射
5. **错误恢复** - 实现重试机制和死信队列
6. **可扩展性** - 通道适配器使用 trait，易于添加新通道

## 9. 安全考虑

1. **API 密钥管理** - 使用环境变量和密钥管理服务
2. **消息加密** - 敏感信息加密存储
3. **速率限制** - 防止滥用
4. **输入验证** - 验证所有用户输入
5. **审计日志** - 记录所有消息和操作
