use actix::prelude::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{debug, error, info};

use crate::chat::model::{MediaType, MessageContent, MessageType, UserMessage};

/// Actor消息类型定义
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SendMessage {
    pub message: UserMessage,
}

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
        MessageContent::RichText { text, format: _ } => text.clone(),
        MessageContent::Structured { data, .. } => {
            serde_json::to_string_pretty(data).unwrap_or_default()
        }
        MessageContent::Media {
            media_type,
            url,
            caption,
            ..
        } => {
            let type_str = match media_type {
                MediaType::Image => "图片",
                MediaType::Video => "视频",
                MediaType::Audio => "音频",
                MediaType::Document => "文档",
                MediaType::Link => "链接",
            };
            match caption {
                Some(caption_text) => format!("[{}] {} - {}", type_str, caption_text, url),
                None => format!("[{}] {}", type_str, url),
            }
        }
        MessageContent::Command { command, args } => {
            if args.is_empty() {
                format!("[命令] {}", command)
            } else {
                format!("[命令] {} {}", command, args.join(" "))
            }
        }
        MessageContent::File {
            filename,
            size,
            url,
            ..
        } => {
            let size_info = size.map(|s| format!(" ({}字节)", s)).unwrap_or_default();
            let url_info = url
                .clone()
                .map(|u| format!(" - URL: {}", u))
                .unwrap_or_default();
            format!("[文件: {}]{}{}", filename, size_info, url_info)
        }
    }
}
// 保存消息到数据库
#[derive(Message)]
#[rtype(result = "Result<SaveMessageResult>")]
pub struct SaveMessage {
    // 消息内容
    pub message: UserMessage,
    // 保存时间
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMessageResult {
    pub message: UserMessage,
}

impl Handler<SaveMessage> for ChannelManagerActor {
    type Result = ResponseFuture<Result<SaveMessageResult>>;

    fn handle(&mut self, msg: SaveMessage, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let message = msg.message;

        Box::pin(async move {
            debug!("📨 处理 SaveMessage: user={}", message.user);

            // 提取内容
            let content_text = extract_content_text(&message.content);

            // 转换 metadata
            let metadata_json = if message.metadata.is_empty() {
                None
            } else {
                Some(serde_json::to_value(&message.metadata)?)
            };

            // 执行插入
            let row = sqlx::query(
                r#"
                INSERT INTO messages ("user", source_ip, content, metadata, created_at)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id
                "#,
            )
            .bind(&message.user)
            .bind(&message.source_ip)
            .bind(content_text)
            .bind(metadata_json)
            .bind(message.created_at)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                error!("❌ 保存失败: {}", e);
                anyhow::anyhow!("保存失败: {}", e)
            })?;

            let id: i32 = row.get("id");
            debug!("✅ 保存成功: id={}, user={}", id, message.user);

            Ok(SaveMessageResult { message })
        })
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub user: String,
    pub limit: Option<i64>,
    pub before: Option<chrono::DateTime<chrono::Utc>>, // 分页游标
}

#[derive(Message)]
#[rtype(result = "Result<Vec<UserMessage>>")]
pub struct GetMessages {
    pub user: String, // 聊天历史通常必须指定用户
    pub before: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: i64,
}

impl Handler<GetMessages> for ChannelManagerActor {
    type Result = ResponseFuture<Result<Vec<UserMessage>>>;

    fn handle(&mut self, msg: GetMessages, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            // 构建分页查询
            // 逻辑：查询用户本人或系统的回复，且时间小于游标
            let rows = sqlx::query(
                r#"
                SELECT "user", source_ip, content, metadata, created_at 
                FROM messages 
                WHERE ("user" = $1 OR "user" = 'ChatAgent')
                  AND ($2::timestamptz IS NULL OR created_at < $2)
                ORDER BY created_at DESC 
                LIMIT $3
                "#,
            )
            .bind(&msg.user)
            .bind(msg.before)
            .bind(msg.limit)
            .fetch_all(&pool)
            .await?;

            // 转换数据
            let mut messages: Vec<UserMessage> = rows
                .into_iter()
                .map(|row| {
                    let content_str: String = row.get("content");
                    let metadata_json: Option<serde_json::Value> = row.get("metadata");

                    UserMessage {
                        user: row.get("user"),
                        source_ip: row.get("source_ip"),
                        content: MessageContent::Text(content_str),
                        message_type: MessageType::Chat,
                        metadata: metadata_json
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
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
