use actix::prelude::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{debug, error, info};

use crate::chat::{
    model::MediaType, model::MessageContent, model::UserMessage,
};

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
