// HTTP 处理函数
use super::model::UserMessage;
use crate::ChannelManagerActor;
use crate::chat::actor_messages::{GetMessages, SaveMessage};
use crate::chat::chat_agent::{ChatAgent, OtherUserMessage};
use crate::chat::model::{MessageContent, MessageType};
use crate::graph_memory::actor_memory::{AgentMemoryHActor, RequestMemory};
use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Add;
use tracing::{debug, error, info, warn};

// 请求结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub user: String,
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 处理通用消息 - 支持新的 UserMessage 格式
#[post("/message")]
pub async fn handle_message(
    chat_request: web::Json<ChatRequest>,
    chat_agent: web::Data<Addr<ChatAgent>>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    memory_manager: web::Data<Addr<AgentMemoryHActor>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let start_time = std::time::Instant::now();
    // 获取客户端IP
    let client_ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let chat_request = match serde_json::from_str::<ChatRequest>(
        &serde_json::to_string(&chat_request).unwrap_or_default(),
    ) {
        Ok(req) => req,
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid JSON format",
                "details": e.to_string()
            })));
        }
    };

    let metadata = chat_request.metadata.clone();

    // 构造 UserMessage
    let user_message = UserMessage {
        user: chat_request.user.clone(),
        content: MessageContent::Text(chat_request.content.clone()),
        message_type: MessageType::Chat,
        source_ip: client_ip.clone(),
        metadata,
        created_at: Utc::now(),
    };

    // 保存消息到数据库
    let db_save_start = std::time::Instant::now();
    let save_message = SaveMessage {
        message: user_message.clone(),
    };

    match channel_manager.send(save_message).await {
        Ok(result) => match result {
            Ok(_) => {
                debug!("消息保存到数据库成功，耗时: {:?}", db_save_start.elapsed());
            }
            Err(e) => {
                warn!(
                    "消息保存到数据库失败: {}，耗时: {:?}",
                    e,
                    db_save_start.elapsed()
                );
            }
        },
        Err(e) => {
            error!("发送保存消息到ChannelManager失败: {}", e);
        }
    }

    let agent_start = std::time::Instant::now();

    let chat_agent_response = chat_agent
        .send(OtherUserMessage {
            content: user_message.clone(),
        })
        .await;
    // 处理 ChatAgent 的响应
    match chat_agent_response {
        Ok(Ok(agent_response)) => {
            // 将ai响应丢给memory
            let _ = memory_manager
                .send(RequestMemory {
                    user_name: agent_response.content.user.clone(),
                    message_content: agent_response.content.content.to_string()
                })
                .await;
            let response_save_start = std::time::Instant::now();
            let save_response_message = SaveMessage {
                message: agent_response.content.clone(),
            };

            match channel_manager.send(save_response_message).await {
                Ok(result) => match result {
                    Ok(_) => {
                        debug!(
                            "ChatAgent响应保存到数据库成功，耗时: {:?}",
                            response_save_start.elapsed()
                        );
                    }
                    Err(e) => {
                        warn!(
                            "ChatAgent响应保存到数据库失败: {}，耗时: {:?}",
                            e,
                            response_save_start.elapsed()
                        );
                    }
                },
                Err(e) => {
                    error!("发送保存ChatAgent响应到ChannelManager失败: {}", e);
                }
            }
            Ok(HttpResponse::Ok().json(agent_response))
        }
        Ok(Err(e)) => {
            let total_duration = start_time.elapsed();
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "ChatAgent error",
                "details": e.to_string(),
                "duration_ms": total_duration.as_millis()
            })))
        }
        Err(e) => {
            let total_duration = start_time.elapsed();

            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Actor communication error",
                "details": e.to_string(),
                "duration_ms": total_duration.as_millis()
            })))
        }
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
    // 必须添加这个字段，对应的 URL 参数是 ?before=2023-10-01T10:00:00Z
    pub before: Option<DateTime<Utc>>,
}

#[get("/message")]
pub async fn get_message_history(
    query: web::Query<HistoryQuery>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    user: web::ReqData<String>,
) -> ActixResult<HttpResponse> {
    // 获取用户名字符串
    let username = user.into_inner();
    // 调用 Actor
    let result = channel_manager
        .send(GetMessages {
            user: username, // 使用中间件解析出的用户名

            before: query.before,
            limit: query.limit.unwrap_or(20),
        })
        .await;

    match result {
        Ok(Ok(messages)) => {
            let formatted_messages: Vec<serde_json::Value> = messages
                .into_iter()
                .map(|m| {
                    let text = match m.content {
                        MessageContent::Text(t) => t,
                        _ => "非文本内容".to_string(),
                    };

                    serde_json::json!({
                        "id": format!("{}-{}", m.user, m.created_at.timestamp_micros()),
                        "role": if m.user == "ChatAgent" { "assistant" } else { "user" },
                        "user": m.user,
                        "content": text,
                        "created_at": m.created_at,
                    })
                })
                .collect();

            Ok(HttpResponse::Ok().json(formatted_messages))
        }
        Ok(Err(e)) => {
            error!("获取历史记录数据库错误: {}", e);
            Ok(HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() })))
        }
        Err(e) => {
            error!("Actor 通信错误: {}", e);
            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}
