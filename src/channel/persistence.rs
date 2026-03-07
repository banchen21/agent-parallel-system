use crate::channel::types::{Message, MessageResult, ProcessingStatus, MessageSource, MessageType, MessagePriority};
use anyhow::Result;
use sqlx::{PgPool, Row};
use serde_json;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 消息持久化管理器
pub struct MessagePersistence {
    pool: PgPool,
}

impl MessagePersistence {
    /// 创建新的持久化管理器
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// 保存消息到数据库
    pub async fn save_message(&self, message: &Message) -> Result<Uuid> {
        let source_str = match message.source {
            MessageSource::Api => "api",
            MessageSource::Terminal => "terminal",
            MessageSource::Internal => "internal",
        };
        
        let type_str = match message.message_type {
            MessageType::Chat => "chat",
            MessageType::Task => "task",
            MessageType::System => "system",
            MessageType::Query => "query",
            MessageType::Response => "response",
        };
        
        let priority_i32 = message.priority.clone() as i32;
        
        let metadata_json = if message.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&message.metadata)?)
        };
        
        let row = sqlx::query(
            r#"
            INSERT INTO messages (
                id, source, message_type, priority, sender, recipient,
                content, metadata, created_at, expires_at, status
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'queued'
            )
            RETURNING id
            "#
        )
        .bind(message.id)
        .bind(source_str)
        .bind(type_str)
        .bind(priority_i32)
        .bind(&message.sender)
        .bind(&message.recipient)
        .bind(&message.content)
        .bind(metadata_json)
        .bind(message.created_at)
        .bind(message.expires_at)
        .fetch_one(&self.pool)
        .await?;
        
        let message_id: uuid::Uuid = row.get("id");
        debug!("消息已保存到数据库: {}", message_id);
        Ok(message_id)
    }
    
    /// 更新消息处理状态
    pub async fn update_message_status(&self, message_id: Uuid, status: ProcessingStatus, result: Option<&MessageResult>) -> Result<()> {
        let status_str = match status {
            ProcessingStatus::Success => "success",
            ProcessingStatus::Failed => "failed",
            ProcessingStatus::Processing => "processing",
            ProcessingStatus::Queued => "queued",
            ProcessingStatus::Rejected => "rejected",
        };
        
        let result_content = result.as_ref().and_then(|r| r.content.as_ref());
        let error_message = result.as_ref().and_then(|r| r.error.as_ref());
        
        sqlx::query(
            r#"
            UPDATE messages
            SET status = $1,
                result_content = $2,
                error_message = $3,
                processed_at = NOW()
            WHERE id = $4
            "#
        )
        .bind(status_str)
        .bind(result_content)
        .bind(error_message)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        
        debug!("消息状态已更新: {} -> {}", message_id, status_str);
        Ok(())
    }
    
    /// 获取消息历史记录
    pub async fn get_message_history(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
        sender: Option<&str>,
        message_type: Option<MessageType>,
    ) -> Result<Vec<Message>> {
        let type_str = message_type.map(|t| match t {
            MessageType::Chat => "chat",
            MessageType::Task => "task",
            MessageType::System => "system",
            MessageType::Query => "query",
            MessageType::Response => "response",
        });
        
        let mut query = "SELECT * FROM messages WHERE 1=1".to_string();
        let mut bind_count = 0;
        
        if sender.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND sender = ${}", bind_count));
        }
        
        if type_str.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND message_type = ${}", bind_count));
        }
        
        query.push_str(" ORDER BY created_at DESC");
        
        if let Some(limit_val) = limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ${}", bind_count));
        }
        
        if let Some(offset_val) = offset {
            bind_count += 1;
            query.push_str(&format!(" OFFSET ${}", bind_count));
        }
        
        let mut query_builder = sqlx::query(&query);
        
        if let Some(sender_val) = sender {
            query_builder = query_builder.bind(sender_val);
        }
        
        if let Some(type_val) = type_str {
            query_builder = query_builder.bind(type_val);
        }
        
        if let Some(limit_val) = limit {
            query_builder = query_builder.bind(limit_val);
        }
        
        if let Some(offset_val) = offset {
            query_builder = query_builder.bind(offset_val);
        }
        
        let rows = query_builder.fetch_all(&self.pool).await?;
        
        let mut messages = Vec::new();
        for row in rows {
            let source: String = row.get("source");
            let source = match source.as_str() {
                "api" => MessageSource::Api,
                "terminal" => MessageSource::Terminal,
                "internal" => MessageSource::Internal,
                _ => continue,
            };
            
            let message_type: String = row.get("message_type");
            let message_type = match message_type.as_str() {
                "chat" => MessageType::Chat,
                "task" => MessageType::Task,
                "system" => MessageType::System,
                "query" => MessageType::Query,
                "response" => MessageType::Response,
                _ => continue,
            };
            
            let priority: i32 = row.get("priority");
            let priority = match priority {
                1 => MessagePriority::Low,
                2 => MessagePriority::Normal,
                3 => MessagePriority::High,
                4 => MessagePriority::Critical,
                _ => MessagePriority::Normal,
            };
            
            let metadata: Option<serde_json::Value> = row.get("metadata");
            let metadata_map = if let Some(serde_json::Value::Object(map)) = metadata {
                map.into_iter().collect()
            } else {
                std::collections::HashMap::new()
            };
            
            let message = Message {
                id: row.get("id"),
                source,
                message_type,
                priority,
                sender: row.get("sender"),
                recipient: row.get("recipient"),
                content: row.get("content"),
                metadata: metadata_map,
                created_at: row.get("created_at"),
                expires_at: row.get("expires_at"),
            };
            
            messages.push(message);
        }
        
        Ok(messages)
    }
    
    /// 获取最近的聊天记录
    pub async fn get_recent_chats(&self, days: i64, limit: Option<i64>) -> Result<Vec<(Message, Option<String>)>> {
        let rows = if let Some(limit_val) = limit {
            sqlx::query(
                r#"
                SELECT
                    m.*,
                    CASE
                        WHEN m.status = 'success' THEN m.result_content
                        ELSE NULL
                    END as response
                FROM messages m
                WHERE m.message_type = 'chat'
                  AND m.created_at > NOW() - INTERVAL '1 day' * $1
                ORDER BY m.created_at DESC
                LIMIT $2
                "#
            )
            .bind(days)
            .bind(limit_val)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    m.*,
                    CASE
                        WHEN m.status = 'success' THEN m.result_content
                        ELSE NULL
                    END as response
                FROM messages m
                WHERE m.message_type = 'chat'
                  AND m.created_at > NOW() - INTERVAL '1 day' * $1
                ORDER BY m.created_at DESC
                "#
            )
            .bind(days)
            .fetch_all(&self.pool)
            .await?
        };
        
        let mut chats = Vec::new();
        for row in rows {
            let source: String = row.get("source");
            let source = match source.as_str() {
                "api" => MessageSource::Api,
                "terminal" => MessageSource::Terminal,
                "internal" => MessageSource::Internal,
                _ => continue,
            };
            
            let message_type: String = row.get("message_type");
            let message_type = match message_type.as_str() {
                "chat" => MessageType::Chat,
                "task" => MessageType::Task,
                "system" => MessageType::System,
                "query" => MessageType::Query,
                "response" => MessageType::Response,
                _ => continue,
            };
            
            let priority: i32 = row.get("priority");
            let priority = match priority {
                1 => MessagePriority::Low,
                2 => MessagePriority::Normal,
                3 => MessagePriority::High,
                4 => MessagePriority::Critical,
                _ => MessagePriority::Normal,
            };
            
            let metadata: Option<serde_json::Value> = row.get("metadata");
            let metadata_map = if let Some(serde_json::Value::Object(map)) = metadata {
                map.into_iter().collect()
            } else {
                std::collections::HashMap::new()
            };
            
            let message = Message {
                id: row.get("id"),
                source,
                message_type,
                priority,
                sender: row.get("sender"),
                recipient: row.get("recipient"),
                content: row.get("content"),
                metadata: metadata_map,
                created_at: row.get("created_at"),
                expires_at: row.get("expires_at"),
            };
            
            let response: Option<String> = row.get("response");
            chats.push((message, response));
        }
        
        Ok(chats)
    }
    
    /// 获取消息统计信息
    pub async fn get_statistics(&self, days: i64) -> Result<MessageStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_messages,
                COUNT(*) FILTER (WHERE status = 'success') as successful_messages,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_messages,
                AVG(EXTRACT(EPOCH FROM (processed_at - created_at))) as avg_processing_time
            FROM messages
            WHERE created_at >= NOW() - INTERVAL '1 day' * $1
            "#
        )
        .bind(days)
        .fetch_one(&self.pool)
        .await?;
        
        let total_messages: i64 = row.get("total_messages");
        let successful_messages: i64 = row.get("successful_messages");
        let failed_messages: i64 = row.get("failed_messages");
        let avg_processing_time: Option<f64> = row.get("avg_processing_time");
        
        Ok(MessageStats {
            total_messages: total_messages as u64,
            successful_messages: successful_messages as u64,
            failed_messages: failed_messages as u64,
            avg_processing_time_seconds: avg_processing_time.unwrap_or(0.0),
        })
    }
    
    /// 清理过期消息
    pub async fn cleanup_expired_messages(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM messages
            WHERE expires_at IS NOT NULL
              AND expires_at < NOW()
              AND status IN ('success', 'failed', 'rejected')
            RETURNING id
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let deleted_count = result.len() as u64;
        if deleted_count > 0 {
            info!("清理了 {} 条过期消息", deleted_count);
        }
        
        Ok(deleted_count)
    }
    
    /// 初始化数据库表
    pub async fn initialize_database(&self) -> Result<()> {
        info!("初始化数据库表结构");
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                source VARCHAR(20) NOT NULL,
                message_type VARCHAR(20) NOT NULL,
                priority INTEGER NOT NULL DEFAULT 2,
                sender VARCHAR(255) NOT NULL,
                recipient VARCHAR(255),
                content TEXT NOT NULL,
                metadata JSONB,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                processed_at TIMESTAMP WITH TIME ZONE,
                expires_at TIMESTAMP WITH TIME ZONE,
                status VARCHAR(20) NOT NULL DEFAULT 'queued',
                error_message TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                result_content TEXT,
                CONSTRAINT valid_source CHECK (source IN ('api', 'terminal', 'internal')),
                CONSTRAINT valid_message_type CHECK (message_type IN ('chat', 'task', 'system', 'query', 'response')),
                CONSTRAINT valid_priority CHECK (priority BETWEEN 1 AND 4),
                CONSTRAINT valid_status CHECK (status IN ('queued', 'processing', 'success', 'failed', 'rejected'))
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // 创建索引
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status)")
            .execute(&self.pool)
            .await?;
        
        info!("数据库表结构初始化完成");
        Ok(())
    }
}

/// 消息统计信息
#[derive(Debug, Clone)]
pub struct MessageStats {
    pub total_messages: u64,
    pub successful_messages: u64,
    pub failed_messages: u64,
    pub avg_processing_time_seconds: f64,
}

impl MessageStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.successful_messages as f64 / self.total_messages as f64 * 100.0
        }
    }
}