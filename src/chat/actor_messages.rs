use actix::prelude::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{error, info};

use crate::chat::model::{MessageContent, UserMessage};

/// 通道层管理器Actor
pub struct ChannelManagerActor {
    pool: sqlx::PgPool,
}

impl Actor for ChannelManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("ChannelManager Actor 已启动");
    }
}

impl ChannelManagerActor {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

// 辅助函数：提取内容文本
fn extract_content_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Multimodal(content_parts) => todo!(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMessageResult {
    pub message: UserMessage,
}

// 保存消息到数据库
#[derive(Message)]
#[rtype(result = "Result<SaveMessageResult>")]
pub struct SaveMessage {
    // 消息内容
    pub message: UserMessage,
}

impl Handler<SaveMessage> for ChannelManagerActor {
    type Result = ResponseFuture<Result<SaveMessageResult>>;

    fn handle(&mut self, msg: SaveMessage, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let user_message = msg.message;

        Box::pin(async move {
            // 提取内容
            let content_text = extract_content_text(&user_message.content);

            // 执行插入
            sqlx::query(
                r#"
                INSERT INTO messages ("user", source_ip, device_type, content, created_at)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id
                "#,
            )
            .bind(&user_message.sender)
            .bind(&user_message.source_ip)
            .bind(&user_message.device_type)
            .bind(content_text)
            .bind(user_message.created_at)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                error!("❌ 保存失败: {}", e);
                anyhow::anyhow!("保存失败: {}", e)
            })?;

            Ok(SaveMessageResult {
                message: user_message,
            })
        })
    }
}

// 返回的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMessage {
    pub user: String,
    pub source_ip: String,
    pub device_type: String,
    pub content: MessageContent,
    pub created_at: DateTime<Utc>,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<ResultMessage>>")]
pub struct GetMessages {
    pub user: String,
    pub ai_name: String,
    pub before: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: i64,
}

impl Handler<GetMessages> for ChannelManagerActor {
    type Result = ResponseFuture<Result<Vec<ResultMessage>>>;

    fn handle(&mut self, msg: GetMessages, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            // 构建分页查询
            // 逻辑：查询用户本人或系统的回复，且时间小于游标
            let rows = sqlx::query(
                r#"
                SELECT "user", source_ip, device_type, content, created_at 
                FROM messages 
                WHERE "user" IN ($1, $2)
                  AND ($3::timestamptz IS NULL OR created_at < $3)
                ORDER BY created_at DESC 
                LIMIT $4
                "#,
            )
            .bind(&msg.user)
            .bind(&msg.ai_name)
            .bind(msg.before)
            .bind(msg.limit)
            .fetch_all(&pool)
            .await?;

            // 转换数据
            let mut messages: Vec<ResultMessage> = rows
                .into_iter()
                .map(|row| {
                    let content_str: String = row.get("content");

                    ResultMessage {
                        user: row.get("user"),
                        source_ip: row.get("source_ip"),
                        device_type: row.get("device_type"),
                        content: MessageContent::Text(content_str),
                        created_at: row.get("created_at"),
                    }
                })
                .collect();

            // 关键：倒序查出来的（最新的在前面），显示时需要反转回正序
            messages.reverse();

            Ok(messages)
        })
    }
}
