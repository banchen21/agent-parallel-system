use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::chat::{ChatSession, ChatMessage, CreateChatSessionRequest, SendChatMessageRequest, LLMConfig},
};

/// 聊天服务
#[derive(Clone)]
pub struct ChatService {
    db_pool: PgPool,
}

impl ChatService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// 创建聊天会话
    pub async fn create_session(
        &self,
        request: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let model = request.model.unwrap_or_else(|| "gpt-3.5-turbo".to_string());
        let temperature = request.temperature.unwrap_or(0.7);
        let max_tokens = request.max_tokens.unwrap_or(2000);
        let context_window = request.context_window.unwrap_or(10);

        let session = sqlx::query_as!(
            ChatSession,
            r#"
            INSERT INTO chat_sessions (
                channel_user_id, title, model, system_prompt,
                temperature, max_tokens, context_window, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            request.channel_user_id,
            request.title,
            model,
            request.system_prompt,
            temperature,
            max_tokens,
            context_window,
            serde_json::json!({})
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(session)
    }

    /// 获取聊天会话
    pub async fn get_session(&self, session_id: Uuid) -> Result<Option<ChatSession>, AppError> {
        let session = sqlx::query_as!(
            ChatSession,
            "SELECT * FROM chat_sessions WHERE id = $1",
            session_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(session)
    }

    /// 获取用户的全局会话（只有一个）
    pub async fn get_or_create_global_session(
        &self,
        channel_user_id: Uuid,
    ) -> Result<ChatSession, AppError> {
        // 尝试获取现有的全局会话
        if let Ok(Some(session)) = sqlx::query_as!(
            ChatSession,
            "SELECT * FROM chat_sessions WHERE channel_user_id = $1 AND is_active = true LIMIT 1",
            channel_user_id
        )
        .fetch_optional(&self.db_pool)
        .await
        {
            return Ok(session);
        }

        // 如果不存在，创建新的全局会话
        let session = sqlx::query_as!(
            ChatSession,
            r#"
            INSERT INTO chat_sessions (
                channel_user_id, title, model, system_prompt,
                temperature, max_tokens, context_window, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            channel_user_id,
            Some("Global Chat Session"),
            "gpt-3.5-turbo",
            None,
            0.7,
            2000,
            10,
            serde_json::json!({"type": "global", "memory_backed": true})
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(session)
    }

    /// 添加聊天消息
    pub async fn add_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        tokens_used: Option<i32>,
    ) -> Result<ChatMessage, AppError> {
        let message = sqlx::query_as!(
            ChatMessage,
            r#"
            INSERT INTO chat_messages (session_id, role, content, tokens_used, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
            session_id,
            role,
            content,
            tokens_used,
            serde_json::json!({})
        )
        .fetch_one(&self.db_pool)
        .await?;

        // 更新会话的 updated_at
        sqlx::query!(
            "UPDATE chat_sessions SET updated_at = NOW() WHERE id = $1",
            session_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(message)
    }

    /// 获取会话的消息历史
    pub async fn get_session_messages(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatMessage>, AppError> {
        let messages = sqlx::query_as!(
            ChatMessage,
            r#"
            SELECT * FROM chat_messages 
            WHERE session_id = $1 
            ORDER BY created_at DESC 
            LIMIT $2
            "#,
            session_id,
            limit
        )
        .fetch_all(&self.db_pool)
        .await?;

        Ok(messages)
    }

    /// 获取默认 LLM 配置
    pub async fn get_default_llm_config(&self) -> Result<Option<LLMConfig>, AppError> {
        let config = sqlx::query_as!(
            LLMConfig,
            "SELECT * FROM llm_configs WHERE is_default = true AND is_active = true LIMIT 1"
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 获取 LLM 配置
    pub async fn get_llm_config(&self, config_id: Uuid) -> Result<Option<LLMConfig>, AppError> {
        let config = sqlx::query_as!(
            LLMConfig,
            "SELECT * FROM llm_configs WHERE id = $1 AND is_active = true",
            config_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 关闭会话
    pub async fn close_session(&self, session_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE chat_sessions SET is_active = false, updated_at = NOW() WHERE id = $1",
            session_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}
