use actix::prelude::*;
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::chat::model::{MessageContent, UserMessage};

/// 通道层管理器Actor
pub struct ChannelManagerActor {
    pool: sqlx::PgPool,
    /// 内部消息队列（用于广播/异步处理）
    queue: VecDeque<UserMessage>,
    /// 订阅者（session_id -> Recipient）
    subscribers: HashMap<String, Recipient<ChannelEvent>>,
}

impl Actor for ChannelManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("ChannelManagerActor started");
        // 周期性广播队列内消息给订阅者
        ctx.run_interval(Duration::from_millis(500), |act, _ctx| {
            if act.queue.is_empty() || act.subscribers.is_empty() {
                return;
            }
            // 把队列快照到本地 vec，然后广播
            let mut batch = Vec::new();
            while let Some(msg) = act.queue.pop_front() {
                batch.push(msg);
            }

            for m in batch.into_iter() {
                let ev = ChannelEvent {
                    user: m.sender.clone(),
                    content: match m.content {
                        MessageContent::Text(ref s) => s.clone(),
                        MessageContent::Multimodal(_) => "<multimodal>".to_string(),
                    },
                    created_at: m.created_at,
                };
                for recipient in act.subscribers.values() {
                    let _ = recipient.do_send(ev.clone());
                }
            }
        });
    }
}

impl ChannelManagerActor {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            queue: VecDeque::new(),
            subscribers: HashMap::new(),
        }
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
        // 兼容旧接口：立即写入数据库，同时把消息放入内部队列用于广播
        self.queue.push_back(user_message.clone());
        Box::pin(async move {
            // 提取内容
            let content_text = extract_content_text(&user_message.content);

            // 执行插入（使用 username 列）
            sqlx::query(
                r#"
                INSERT INTO channel_messages (username, source_ip, device_type, content)
                VALUES ($1, $2, $3, $4)
                RETURNING id
                "#,
            )
            .bind(&user_message.sender)
            .bind(&user_message.source_ip)
            .bind(&user_message.device_type)
            .bind(content_text)
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

/// 队列事件：广播到订阅者的消息事件
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ChannelEvent {
    pub user: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

// 订阅/退订消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct SubscribeChannelNotify {
    pub session_id: String,
    pub recipient: Recipient<ChannelEvent>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UnsubscribeChannelNotify {
    pub session_id: String,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<ResultMessage>>")]
pub struct GetMessages {
    pub user: String,
    pub ai_name: String,
    pub before: Option<DateTime<Utc>>,
    pub limit: i64,
}

impl Handler<GetMessages> for ChannelManagerActor {
    type Result = ResponseFuture<Result<Vec<ResultMessage>>>;

    fn handle(&mut self, msg: GetMessages, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                                SELECT username, source_ip, device_type, content, created_at
                                FROM channel_messages
                                WHERE (username = $1 OR username = $2 OR username = '任务系统')
                                    AND ($3 IS NULL OR created_at < $3)
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
                        user: row.get("username"),
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

// 订阅管理处理
impl Handler<SubscribeChannelNotify> for ChannelManagerActor {
    type Result = ();

    fn handle(&mut self, msg: SubscribeChannelNotify, _ctx: &mut Self::Context) -> Self::Result {
        self.subscribers.insert(msg.session_id, msg.recipient);
    }
}

impl Handler<UnsubscribeChannelNotify> for ChannelManagerActor {
    type Result = ();

    fn handle(&mut self, msg: UnsubscribeChannelNotify, _ctx: &mut Self::Context) -> Self::Result {
        self.subscribers.remove(&msg.session_id);
    }
}
