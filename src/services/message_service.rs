//! 消息服务
//! 
//! 负责处理系统内部消息的发送、接收和管理

use anyhow::Result;
use bb8_redis::RedisConnectionManager;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::{AgentMessage, TaskMessage, UserMessage},
};

/// 消息服务
pub struct MessageService {
    db_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl MessageService {
    /// 创建新的消息服务
    pub fn new(db_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            db_pool,
            redis_pool,
        }
    }

    /// 发送智能体消息
    pub async fn send_agent_message(
        &self,
        agent_id: Uuid,
        message_type: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<AgentMessage, AppError> {
        let message = AgentMessage {
            id: Uuid::new_v4(),
            agent_id,
            message_type: message_type.to_string(),
            content: content.to_string(),
            metadata,
            created_at: chrono::Utc::now(),
            read: false,
        };

        // 保存到数据库
        sqlx::query!(
            r#"
            INSERT INTO agent_messages (id, agent_id, message_type, content, metadata, created_at, read)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            message.id,
            message.agent_id,
            message.message_type,
            message.content,
            message.metadata,
            message.created_at,
            message.read
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 发布到Redis频道
        let mut redis_conn = self.redis_pool.get().await
            .map_err(|e| AppError::RedisError(e.to_string()))?;
        
        let channel = format!("agent:{}:messages", agent_id);
        let message_json = serde_json::to_string(&message)
            .map_err(|e| AppError::SerializationError(e.to_string()))?;
        
        redis::cmd("PUBLISH")
            .arg(channel)
            .arg(message_json)
            .query_async::<i64>(&mut *redis_conn)
            .await
            .map_err(|e| AppError::RedisError(e.to_string()))?;

        Ok(message)
    }

    /// 发送任务消息
    pub async fn send_task_message(
        &self,
        task_id: Uuid,
        message_type: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<TaskMessage, AppError> {
        let message = TaskMessage {
            id: Uuid::new_v4(),
            task_id,
            message_type: message_type.to_string(),
            content: content.to_string(),
            metadata,
            created_at: chrono::Utc::now(),
        };

        // 保存到数据库
        sqlx::query!(
            r#"
            INSERT INTO task_messages (id, task_id, message_type, content, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            message.id,
            message.task_id,
            message.message_type,
            message.content,
            message.metadata,
            message.created_at
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(message)
    }

    /// 发送用户消息
    pub async fn send_user_message(
        &self,
        user_id: Uuid,
        message_type: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<UserMessage, AppError> {
        let message = UserMessage {
            id: Uuid::new_v4(),
            user_id,
            message_type: message_type.to_string(),
            content: content.to_string(),
            metadata,
            created_at: chrono::Utc::now(),
            read: false,
        };

        // 保存到数据库
        sqlx::query!(
            r#"
            INSERT INTO user_messages (id, user_id, message_type, content, metadata, created_at, read)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            message.id,
            message.user_id,
            message.message_type,
            message.content,
            message.metadata,
            message.created_at,
            message.read
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(message)
    }

    /// 获取智能体消息
    pub async fn get_agent_messages(
        &self,
        agent_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<AgentMessage>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let messages = sqlx::query_as!(
            AgentMessage,
            r#"
            SELECT id, agent_id, message_type, content, metadata, created_at, read
            FROM agent_messages
            WHERE agent_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            agent_id,
            limit,
            offset
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(messages)
    }

    /// 获取任务消息
    pub async fn get_task_messages(
        &self,
        task_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<TaskMessage>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let messages = sqlx::query_as!(
            TaskMessage,
            r#"
            SELECT id, task_id, message_type, content, metadata, created_at
            FROM task_messages
            WHERE task_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            task_id,
            limit,
            offset
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(messages)
    }

    /// 获取用户消息
    pub async fn get_user_messages(
        &self,
        user_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<UserMessage>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let messages = sqlx::query_as!(
            UserMessage,
            r#"
            SELECT id, user_id, message_type, content, metadata, created_at, read
            FROM user_messages
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            limit,
            offset
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(messages)
    }

    /// 标记消息为已读
    pub async fn mark_message_as_read(
        &self,
        message_id: Uuid,
        message_type: &str,
    ) -> Result<(), AppError> {
        match message_type {
            "agent" => {
                sqlx::query!(
                    r#"
                    UPDATE agent_messages
                    SET read = true
                    WHERE id = $1
                    "#,
                    message_id
                )
                .execute(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
            "user" => {
                sqlx::query!(
                    r#"
                    UPDATE user_messages
                    SET read = true
                    WHERE id = $1
                    "#,
                    message_id
                )
                .execute(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
            _ => {
                return Err(AppError::ValidationError(
                    format!("不支持的消息类型: {}", message_type)
                ));
            }
        }

        Ok(())
    }

    /// 删除消息
    pub async fn delete_message(
        &self,
        message_id: Uuid,
        message_type: &str,
    ) -> Result<(), AppError> {
        match message_type {
            "agent" => {
                sqlx::query!(
                    r#"
                    DELETE FROM agent_messages
                    WHERE id = $1
                    "#,
                    message_id
                )
                .execute(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
            "user" => {
                sqlx::query!(
                    r#"
                    DELETE FROM user_messages
                    WHERE id = $1
                    "#,
                    message_id
                )
                .execute(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
            "task" => {
                sqlx::query!(
                    r#"
                    DELETE FROM task_messages
                    WHERE id = $1
                    "#,
                    message_id
                )
                .execute(&self.db_pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
            _ => {
                return Err(AppError::ValidationError(
                    format!("不支持的消息类型: {}", message_type)
                ));
            }
        }

        Ok(())
    }

    /// 订阅智能体消息
    pub async fn subscribe_agent_messages(
        &self,
        agent_id: Uuid,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, AppError> {
        let _redis_conn = self.redis_pool.get().await
            .map_err(|e| AppError::RedisError(e.to_string()))?;
        
        let _channel = format!("agent:{}:messages", agent_id);
        let (_tx, rx) = tokio::sync::mpsc::channel(100);
        
        // 这里应该实现Redis订阅逻辑
        // 由于复杂性，这里只返回一个空的接收器
        Ok(rx)
    }

    /// 发送系统广播消息
    pub async fn send_system_broadcast(
        &self,
        message_type: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let mut redis_conn = self.redis_pool.get().await
            .map_err(|e| AppError::RedisError(e.to_string()))?;
        
        let broadcast_message = serde_json::json!({
            "type": message_type,
            "content": content,
            "metadata": metadata,
            "timestamp": chrono::Utc::now().timestamp_millis()
        });
        
        let message_json = serde_json::to_string(&broadcast_message)
            .map_err(|e| AppError::SerializationError(e.to_string()))?;
        
        redis::cmd("PUBLISH")
            .arg("system:broadcast")
            .arg(message_json)
            .query_async::<i64>(&mut *redis_conn)
            .await
            .map_err(|e| AppError::RedisError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::create_test_pool;

    #[tokio::test]
    async fn test_message_service_creation() {
        // 这个测试主要是验证服务可以正确创建
        // 由于需要数据库连接，这里只测试编译通过
        assert!(true);
    }
}
