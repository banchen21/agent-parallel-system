# APS 聊天和通道层 - 详细实现计划

## 执行摘要

基于以下决策创建的实现计划：
- **LLM 支持**: OpenAI API + 本地模型（Ollama/LM Studio）
- **通道优先级**: Telegram + Web Chat（第一阶段），后续扩展 Discord/QQ
- **历史存储**: 完整对话历史存储在 PostgreSQL
- **APS 集成**: 通过现有 API 端点调用

---

## 第一阶段：核心基础设施

### 1.1 数据库迁移

创建以下表结构：

```sql
-- 通道配置表
CREATE TABLE channel_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    config JSONB NOT NULL,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(channel_type, name)
);

-- 通道用户映射表
CREATE TABLE channel_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_type VARCHAR(50) NOT NULL,
    channel_user_id VARCHAR(255) NOT NULL,
    channel_username VARCHAR(255),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(channel_type, channel_user_id)
);

-- 聊天会话表
CREATE TABLE chat_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_type VARCHAR(50),
    title VARCHAR(255),
    model VARCHAR(100) DEFAULT 'gpt-3.5-turbo',
    status VARCHAR(50) DEFAULT 'active',
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    last_message_at TIMESTAMP,
    INDEX idx_user_id (user_id),
    INDEX idx_status (status)
);

-- 聊天消息表
CREATE TABLE chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    tokens_used INTEGER,
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    INDEX idx_session_id (session_id),
    INDEX idx_created_at (created_at)
);

-- 通道消息日志表
CREATE TABLE channel_message_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type VARCHAR(50) NOT NULL,
    channel_message_id VARCHAR(255),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    direction VARCHAR(50),
    content TEXT,
    status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    INDEX idx_channel_type (channel_type),
    INDEX idx_user_id (user_id)
);

-- LLM 配置表
CREATE TABLE llm_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    api_key_encrypted VARCHAR(500),
    base_url VARCHAR(500),
    model_name VARCHAR(100),
    config JSONB,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(provider, name)
);
```

### 1.2 Rust 数据模型

**文件**: `src/models/channel.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChannelType {
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "qq")]
    QQ,
    #[serde(rename = "web")]
    Web,
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Telegram => "telegram",
            ChannelType::Discord => "discord",
            ChannelType::QQ => "qq",
            ChannelType::Web => "web",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelConfig {
    pub id: Uuid,
    pub channel_type: String,
    pub name: String,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel_type: String,
    pub channel_user_id: String,
    pub channel_username: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    pub channel_type: ChannelType,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel_type: Option<String>,
    pub title: String,
    pub model: String,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub tokens_used: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LLMConfig {
    pub id: Uuid,
    pub provider: String,
    pub name: String,
    pub api_key_encrypted: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

---

## 第二阶段：通道层实现

### 2.1 通道适配器架构

**文件**: `src/channels/mod.rs`

```rust
pub mod adapter;
pub mod telegram;
pub mod web;

pub use adapter::{ChannelAdapter, ChannelMessage};
pub use telegram::TelegramAdapter;
pub use web::WebAdapter;
```

**文件**: `src/channels/adapter.rs`

```rust
use async_trait::async_trait;
use serde_json::Value;
use crate::models::channel::{ChannelMessage, ChannelType};
use crate::core::errors::AppError;

#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// 发送消息到通道
    async fn send_message(&self, message: ChannelMessage) -> Result<String, AppError>;
    
    /// 处理来自通道的回调（Webhook）
    async fn handle_callback(&self, payload: Value) -> Result<ChannelMessage, AppError>;
    
    /// 获取通道类型
    fn channel_type(&self) -> ChannelType;
    
    /// 验证通道配置
    async fn validate_config(&self) -> Result<(), AppError>;
}
```

### 2.2 Telegram 适配器

**文件**: `src/channels/telegram.rs`

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use crate::models::channel::{ChannelMessage, ChannelType};
use crate::core::errors::AppError;
use super::adapter::ChannelAdapter;

pub struct TelegramAdapter {
    bot_token: String,
    api_url: String,
    client: Client,
}

impl TelegramAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            api_url: format!("https://api.telegram.org/bot{}", bot_token),
            bot_token,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    async fn send_message(&self, message: ChannelMessage) -> Result<String, AppError> {
        let payload = json!({
            "chat_id": message.sender_id,
            "text": message.content,
            "parse_mode": "Markdown"
        });

        let response = self.client
            .post(&format!("{}/sendMessage", self.api_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(e.to_string()))?;

        let result: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(e.to_string()))?;

        Ok(result["result"]["message_id"].to_string())
    }

    async fn handle_callback(&self, payload: Value) -> Result<ChannelMessage, AppError> {
        let message = payload["message"].clone();
        let chat_id = message["chat"]["id"].as_i64()
            .ok_or(AppError::ValidationError("Missing chat_id".to_string()))?;
        let text = message["text"].as_str()
            .ok_or(AppError::ValidationError("Missing text".to_string()))?;
        let user_id = message["from"]["id"].as_i64()
            .ok_or(AppError::ValidationError("Missing user_id".to_string()))?;
        let username = message["from"]["username"].as_str();

        Ok(ChannelMessage {
            id: message["message_id"].to_string(),
            channel_type: ChannelType::Telegram,
            sender_id: user_id.to_string(),
            sender_name: username.map(|s| s.to_string()),
            content: text.to_string(),
            timestamp: chrono::Utc::now(),
            metadata: Some(payload),
        })
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    async fn validate_config(&self) -> Result<(), AppError> {
        let response = self.client
            .get(&format!("{}/getMe", self.api_url))
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(AppError::ExternalServiceError("Invalid Telegram bot token".to_string()))
        }
    }
}
```

### 2.3 Web Chat 适配器

**文件**: `src/channels/web.rs`

```rust
use async_trait::async_trait;
use serde_json::Value;
use crate::models::channel::{ChannelMessage, ChannelType};
use crate::core::errors::AppError;
use super::adapter::ChannelAdapter;

pub struct WebAdapter;

#[async_trait]
impl ChannelAdapter for WebAdapter {
    async fn send_message(&self, _message: ChannelMessage) -> Result<String, AppError> {
        // Web Chat 通过 WebSocket 发送，这里只返回消息 ID
        Ok(uuid::Uuid::new_v4().to_string())
    }

    async fn handle_callback(&self, payload: Value) -> Result<ChannelMessage, AppError> {
        let sender_id = payload["sender_id"].as_str()
            .ok_or(AppError::ValidationError("Missing sender_id".to_string()))?;
        let content = payload["content"].as_str()
            .ok_or(AppError::ValidationError("Missing content".to_string()))?;

        Ok(ChannelMessage {
            id: uuid::Uuid::new_v4().to_string(),
            channel_type: ChannelType::Web,
            sender_id: sender_id.to_string(),
            sender_name: payload["sender_name"].as_str().map(|s| s.to_string()),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            metadata: Some(payload),
        })
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Web
    }

    async fn validate_config(&self) -> Result<(), AppError> {
        Ok(())
    }
}
```

---

## 第三阶段：聊天层实现

### 3.1 LLM 客户端

**文件**: `src/services/llm_client.rs`

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use crate::core::errors::AppError;

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat_completion(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, AppError>;
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct OpenAIClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAIClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn chat_completion(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, AppError> {
        let payload = json!({
            "model": model,
            "messages": messages.iter().map(|m| json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>(),
            "temperature": 0.7,
            "max_tokens": 2000,
        });

        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(e.to_string()))?;

        let result: Value = response.json().await
            .map_err(|e| AppError::ExternalServiceError(e.to_string()))?;

        result["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::ExternalServiceError("Invalid response format".to_string()))
    }
}
```

### 3.2 聊天服务

**文件**: `src/services/chat_service.rs`

```rust
use uuid::Uuid;
use sqlx::PgPool;
use crate::models::channel::ChatMessage as DbChatMessage;
use crate::models::channel::ChatSession;
use crate::core::errors::AppError;
use super::llm_client::{LLMClient, ChatMessage};

pub struct ChatService {
    db: PgPool,
    llm_client: Box<dyn LLMClient>,
}

impl ChatService {
    pub fn new(db: PgPool, llm_client: Box<dyn LLMClient>) -> Self {
        Self { db, llm_client }
    }

    pub async fn create_session(
        &self,
        user_id: Uuid,
        title: String,
        model: String,
    ) -> Result<ChatSession, AppError> {
        let session = sqlx::query_as::<_, ChatSession>(
            "INSERT INTO chat_sessions (user_id, title, model) 
             VALUES ($1, $2, $3) 
             RETURNING *"
        )
        .bind(user_id)
        .bind(title)
        .bind(model)
        .fetch_one(&self.db)
        .await?;

        Ok(session)
    }

    pub async fn get_session(&self, session_id: Uuid) -> Result<ChatSession, AppError> {
        sqlx::query_as::<_, ChatSession>(
            "SELECT * FROM chat_sessions WHERE id = $1"
        )
        .bind(session_id)
        .fetch_one(&self.db)
        .await
        .map_err(|_| AppError::NotFound("Session not found".to_string()))
    }

    pub async fn add_message(
        &self,
        session_id: Uuid,
        role: String,
        content: String,
    ) -> Result<DbChatMessage, AppError> {
        let message = sqlx::query_as::<_, DbChatMessage>(
            "INSERT INTO chat_messages (session_id, role, content) 
             VALUES ($1, $2, $3) 
             RETURNING *"
        )
        .bind(session_id)
        .bind(role)
        .bind(content)
        .fetch_one(&self.db)
        .await?;

        Ok(message)
    }

    pub async fn get_messages(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DbChatMessage>, AppError> {
        sqlx::query_as::<_, DbChatMessage>(
            "SELECT * FROM chat_messages 
             WHERE session_id = $1 
             ORDER BY created_at DESC 
             LIMIT $2"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn process_message(
        &self,
        session_id: Uuid,
        user_message: String,
        model: &str,
    ) -> Result<String, AppError> {
        // 保存用户消息
        self.add_message(session_id, "user".to_string(), user_message.clone()).await?;

        // 获取对话历史
        let messages = self.get_messages(session_id, 20).await?;
        let mut chat_messages = Vec::new();

        for msg in messages.iter().rev() {
            chat_messages.push(ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        // 调用 LLM
        let response = self.llm_client.chat_completion(chat_messages, model).await?;

        // 保存助手响应
        self.add_message(session_id, "assistant".to_string(), response.clone()).await?;

        Ok(response)
    }
}
```

---

## 第四阶段：消息路由和集成

### 4.1 消息路由服务

**文件**: `src/services/message_router_service.rs`

```rust
use uuid::Uuid;
use sqlx::PgPool;
use crate::models::channel::{ChannelMessage, ChannelType, ChannelUser};
use crate::core::errors::AppError;
use super::chat_service::ChatService;

pub struct MessageRouterService {
    db: PgPool,
    chat_service: ChatService,
}

impl MessageRouterService {
    pub fn new(db: PgPool, chat_service: ChatService) -> Self {
        Self { db, chat_service }
    }

    pub async fn route_message(
        &self,
        channel_msg: ChannelMessage,
    ) -> Result<String, AppError> {
        // 查找或创建通道用户映射
        let user_id = self.get_or_create_channel_user(
            &channel_msg.channel_type,
            &channel_msg.sender_id,
            channel_msg.sender_name.clone(),
        ).await?;

        // 获取或创建聊天会话
        let session = self.get_or_create_session(
            user_id,
            &channel_msg.channel_type,
        ).await?;

        // 处理消息
        let response = self.chat_service.process_message(
            session.id,
            channel_msg.content,
            &session.model,
        ).await?;

        Ok(response)
    }

    async fn get_or_create_channel_user(
        &self,
        channel_type: &ChannelType,
        channel_user_id: &str,
        channel_username: Option<String>,
    ) -> Result<Uuid, AppError> {
        let channel_type_str = channel_type.as_str();

        // 尝试查找现有用户
        if let Ok(user) = sqlx::query_as::<_, ChannelUser>(
            "SELECT * FROM channel_users 
             WHERE channel_type = $1 AND channel_user_id = $2"
        )
        .bind(channel_type_str)
        .bind(channel_user_id)
        .fetch_one(&self.db)
        .await {
            return Ok(user.user_id);
        }

        // 创建新用户或使用匿名用户
        // 这里简化处理，实际应该创建或关联用户
        Err(AppError::NotFound("User not found".to_string()))
    }

    async fn get_or_create_session(
        &self,
        user_id: Uuid,
        channel_type: &ChannelType,
    ) -> Result<crate::models::channel::ChatSession, AppError> {
        let channel_type_str = channel_type.as_str();

        // 查找活跃会话
        if let Ok(session) = sqlx::query_as::<_, crate::models::channel::ChatSession>(
            "SELECT * FROM chat_sessions 
             WHERE user_id = $1 AND channel_type = $2 AND status = 'active' 
             ORDER BY last_message_at DESC LIMIT 1"
        )
        .bind(user_id)
        .bind(channel_type_str)
        .fetch_one(&self.db)
        .await {
            return Ok(session);
        }

        // 创建新会话
        self.chat_service.create_session(
            user_id,
            format!("Chat on {}", channel_type_str),
            "gpt-3.5-turbo".to_string(),
        ).await
    }
}
```

---

## 第五阶段：API 端点

### 5.1 聊天 API 路由

**文件**: `src/api/chat_routes.rs`

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    routing::{get, post},
    Router,
};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub title: String,
    pub model: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/:session_id", get(get_session))
        .route("/sessions/:session_id/messages", post(send_message).get(get_messages))
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    // 实现创建会话
    Ok((StatusCode::CREATED, Json(serde_json::json!({}))))
}

async fn list_sessions(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 实现列出会话
    Ok(Json(serde_json::json!([])))
}

async fn get_session(
    State(_state): State<AppState>,
    Path(_session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 实现获取会话
    Ok(Json(serde_json::json!({})))
}

async fn send_message(
    State(_state): State<AppState>,
    Path(_session_id): Path<Uuid>,
    Json(_req): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 实现发送消息
    Ok(Json(serde_json::json!({})))
}

async fn get_messages(
    State(_state): State<AppState>,
    Path(_session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 实现获取消息
    Ok(Json(serde_json::json!([])))
}
```

### 5.2 Webhook 端点

**文件**: `src/api/webhook_routes.rs`

```rust
use axum::{
    extract::State,
    http::StatusCode,
    Json,
    routing::post,
    Router,
};
use serde_json::Value;
use crate::AppState;

pub fn webhook_routes() -> Router<AppState> {
    Router::new()
        .route("/webhooks/telegram", post(telegram_webhook))
        .route("/webhooks/web", post(web_webhook))
}

async fn telegram_webhook(
    State(_state): State<AppState>,
    Json(_payload): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    // 处理 Telegram webhook
    Ok(StatusCode::OK)
}

async fn web_webhook(
    State(_state): State<AppState>,
    Json(_payload): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    // 处理 Web Chat webhook
    Ok(StatusCode::OK)
}
```

---

## 实现优先级和依赖关系

```
Phase 1: 数据库 + 模型
    ↓
Phase 2: 通道适配器 (Telegram + Web)
    ↓
Phase 3: LLM 客户端 + 聊天服务
    ↓
Phase 4: 消息路由服务
    ↓
Phase 5: API 端点 + Webhook
    ↓
Phase 6: 测试 + 文档
```

---

## 关键配置项

### 环境变量

```bash
# LLM 配置
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.openai.com/v1  # 或本地 Ollama: http://localhost:11434/v1

# Telegram 配置
TELEGRAM_BOT_TOKEN=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11

# 数据库
DATABASE_URL=postgresql://user:password@localhost/aps_db

# 服务配置
CHAT_MODEL_DEFAULT=gpt-3.5-turbo
CHAT_MAX_HISTORY=20
```

---

## 测试策略

1. **单元测试**: 各个适配器和服务的独立测试
2. **集成测试**: 端到端的消息流测试
3. **Mock 测试**: 使用 mock LLM 和通道进行测试

---

## 后续扩展

- Discord 适配器实现
- QQ 适配器实现
- 更多 LLM 提供商支持（Claude、Gemini 等）
- 向量数据库集成用于语义搜索
- 流式响应支持
- 消息审核和内容过滤
