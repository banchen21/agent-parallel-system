// HTTP 处理函数
use super::model::UserMessage;
use crate::chat::chat_agent::{ChatAgent, OtherUserMessage};
use crate::chat::model::{MessageContent, MessageType};
use crate::{ChannelManagerActor, channel::actor_manager::SaveMessage};
use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

// 请求结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub user: String,
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 处理通用消息 - 支持新的 UserMessage 格式
pub async fn handle_message(
    chat_request: web::Json<ChatRequest>,
    chat_agent: web::Data<Addr<ChatAgent>>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let start_time = std::time::Instant::now();
    // 获取客户端IP
    let client_ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    debug!("🌍 客户端IP: {}", client_ip);

    // 解析JSON请求体
    debug!("📥 开始解析JSON请求体...");
    let chat_request = match serde_json::from_str::<ChatRequest>(
        &serde_json::to_string(&chat_request).unwrap_or_default(),
    ) {
        Ok(req) => {
            debug!(
                "✅ JSON解析成功: user={}, content_length={}, metadata_keys={:?}",
                req.user,
                req.content.len(),
                req.metadata.keys().collect::<Vec<_>>()
            );
            req
        }
        Err(e) => {
            error!("❌ JSON解析失败: {}", e);
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid JSON format",
                "details": e.to_string()
            })));
        }
    };

    let metadata = chat_request.metadata.clone();

    // 构造 UserMessage
    debug!("🏗️ 构造UserMessage...");
    let user_message = UserMessage {
        user: chat_request.user.clone(),
        content: MessageContent::Text(chat_request.content.clone()),
        message_type: MessageType::Chat,
        source_ip: client_ip.clone(),
        metadata,
        created_at: Utc::now(),
    };

    debug!(
        "📝 UserMessage构造完成: user={}, message_type={:?}, source_ip={}",
        user_message.user, user_message.message_type, user_message.source_ip
    );

    // 保存消息到数据库
    debug!("💾 开始保存消息到数据库...");
    let db_save_start = std::time::Instant::now();
    let save_message = SaveMessage {
        message: user_message.clone(),
    };

    match channel_manager.send(save_message).await {
        Ok(result) => match result {
            Ok(_) => {
                debug!(
                    "✅ 消息保存到数据库成功，耗时: {:?}",
                    db_save_start.elapsed()
                );
            }
            Err(e) => {
                warn!(
                    "⚠️ 消息保存到数据库失败: {}，耗时: {:?}",
                    e,
                    db_save_start.elapsed()
                );
            }
        },
        Err(e) => {
            error!("❌ 发送保存消息到ChannelManager失败: {}", e);
        }
    }

    // 发送消息给 ChatAgent 处理
    debug!("🤖 开始发送消息给ChatAgent处理...");
    let agent_start = std::time::Instant::now();

    // 添加处理状态日志，让前端知道正在处理
    debug!("⏳ 正在处理用户消息，请稍候...");

    let chat_agent_response = chat_agent
        .send(OtherUserMessage {
            content: user_message.clone(),
        })
        .await;

    debug!("⏱️ ChatAgent处理完成， 预览响应: {:?}", chat_agent_response);

    let agent_duration = agent_start.elapsed();
    debug!("⏱️ ChatAgent处理耗时: {:?}", agent_duration);

    // 处理 ChatAgent 的响应
    match chat_agent_response {
        Ok(Ok(agent_response)) => {
            let response_save_start = std::time::Instant::now();
            let save_response_message = SaveMessage {
                message: agent_response.content.clone(),
            };

            match channel_manager.send(save_response_message).await {
                Ok(result) => match result {
                    Ok(_) => {
                        debug!(
                            "✅ ChatAgent响应保存到数据库成功，耗时: {:?}",
                            response_save_start.elapsed()
                        );
                    }
                    Err(e) => {
                        warn!(
                            "⚠️ ChatAgent响应保存到数据库失败: {}，耗时: {:?}",
                            e,
                            response_save_start.elapsed()
                        );
                    }
                },
                Err(e) => {
                    error!("❌ 发送保存ChatAgent响应到ChannelManager失败: {}", e);
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

// 获取消息历史
pub async fn get_message_history() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "消息历史接口 - 待实现"
    })))
}