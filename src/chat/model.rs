use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 用户发消息过来的消息结构体 - 兼容各种格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// 用户标识
    pub user: String,
    /// 消息来源(ip地址)
    pub source_ip: String,
    /// 消息内容 - 支持多种格式
    pub content: MessageContent,
    /// 消息类型（可选，默认为Chat）
    #[serde(default = "default_message_type")]
    pub message_type: MessageType,
    /// 附加元数据
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl UserMessage {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Chat,
    Task,
}

/// 消息内容 - 支持多种格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本
    Text(String),
    /// 富文本格式
    RichText { text: String, format: TextFormat },
    /// 结构化消息
    Structured {
        message_type: String,
        data: serde_json::Value,
    },
    /// 多媒体消息
    Media {
        media_type: MediaType,
        url: String,
        caption: Option<String>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    /// 命令消息
    Command { command: String, args: Vec<String> },
    /// 文件消息
    File {
        filename: String,
        content_type: String,
        size: Option<u64>,
        url: Option<String>,
    },
}

/// 文本格式类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextFormat {
    Plain,
    Markdown,
    Html,
    Json,
}

/// 媒体类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Link,
}

impl Default for MessageType {
    fn default() -> Self {
        MessageType::Chat
    }
}

fn default_message_type() -> MessageType {
    MessageType::Chat
}

impl ToString for MessageContent {
    fn to_string(&self) -> String {
        match self {
            MessageContent::Text(text) => text.clone(),
            MessageContent::RichText { text, format } => match format {
                TextFormat::Plain => text.clone(),
                TextFormat::Markdown => format!("{}\n[Markdown格式]", text),
                TextFormat::Html => format!("{}\n[HTML格式]", text),
                TextFormat::Json => format!("{}\n[JSON格式]", text),
            },
            MessageContent::Structured { message_type, data } => {
                format!(
                    "[结构化消息 - 类型: {}]\n{}",
                    message_type,
                    serde_json::to_string_pretty(data).unwrap_or_default()
                )
            }
            MessageContent::Media {
                media_type,
                url,
                caption,
                metadata: _,
            } => {
                let media_desc = match media_type {
                    MediaType::Image => "图片",
                    MediaType::Video => "视频",
                    MediaType::Audio => "音频",
                    MediaType::Document => "文档",
                    MediaType::Link => "链接",
                };
                match caption {
                    Some(caption_text) => {
                        format!("[{}: {}] {}\nURL: {}", media_desc, caption_text, url, url)
                    }
                    None => format!("[{}] {}\nURL: {}", media_desc, url, url),
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
                content_type,
                size,
                url,
            } => {
                let size_info = size.map(|s| format!(" ({}字节)", s)).unwrap_or_default();
                let url_info = url
                    .clone()
                    .map(|u| format!("\nURL: {}", u))
                    .unwrap_or_default();
                format!(
                    "[文件: {}] [类型: {}]{}\n{}",
                    filename, content_type, size_info, url_info
                )
            }
        }
    }
}
