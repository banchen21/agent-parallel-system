// HTTP 处理函数
use super::model::UserMessage;
use crate::ChannelManagerActor;
use crate::chat::actor_messages::{GetMessages, ResultMessage, SaveMessage};
use crate::chat::chat_agent::{ChatAgent, OtherUserMessage};
use crate::chat::model::MessageContent;
use crate::graph_memory::actor_memory::{AgentMemoryHActor, GetMyName, RequestMemory};
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
    pub device_type: String,
    pub content: MessageContent,
    pub created_at: DateTime<Utc>,
}

/// 处理通用消息 - 支持新的 UserMessage 格式
#[post("/message")]
pub async fn handle_message(
    chat_request: web::Json<ChatRequest>,
    chat_agent: web::Data<Addr<ChatAgent>>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    agent_memory_hactor: web::Data<Addr<AgentMemoryHActor>>,
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

    // 构造 UserMessage
    let user_message = UserMessage {
        sender: chat_request.user,
        source_ip: client_ip,
        device_type: chat_request.device_type,
        content: chat_request.content,
        created_at: chat_request.created_at,
    };

    let chat_agent_response = chat_agent
        .send(OtherUserMessage {
            content: user_message.clone(),
        })
        .await;
    // 处理 ChatAgent 的响应
    let ai_name = agent_memory_hactor.send(GetMyName {}).await.unwrap();

    match chat_agent_response {
        Ok(Ok(agent_response)) => {
            let channel_manager_clone = channel_manager.get_ref().clone();
            let agent_memory_clone = agent_memory_hactor.get_ref().clone();
            let sender_name = user_message.sender.clone();
            let ai_name_clone = ai_name.clone();
            let agent_content_debug = format!("{:?}", agent_response.content);
            tokio::spawn(async move {
                debug!("开始异步处理记忆反思...");
                let memory_content_short: Vec<ResultMessage> = match channel_manager_clone
                    .send(GetMessages {
                        user: sender_name,
                        ai_name: ai_name_clone.clone(),
                        before: None,
                        limit: 20,
                    })
                    .await
                {
                    Ok(Ok(msgs)) => msgs,
                    Ok(Err(e)) => {
                        error!("❌ 异步获取消息历史失败: {}", e);
                        vec![]
                    }
                    Err(e) => {
                        error!("❌ 异步获取消息历史通信错误: {}", e);
                        vec![]
                    }
                };

                let mem_request = RequestMemory {
                    user_name: ai_name_clone,
                    momory_content_short: memory_content_short,
                    message_content: MessageContent::Text(agent_content_debug),
                };

                if let Err(e) = agent_memory_clone.send(mem_request).await {
                    error!("❌ 异步发送 RequestMemory 失败: {}", e);
                } else {
                    debug!("✅ 异步记忆反思处理完成");
                }
            });
            // --- 异步处理结束 ---

            // 保存消息到数据库
            let db_save_start = std::time::Instant::now();
            let save_message = SaveMessage {
                message: user_message.clone(),
                created_at: Utc::now(),
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
            for message in agent_response.content.iter().cloned() {
                let save_message = SaveMessage {
                    message: UserMessage {
                        sender: ai_name.clone(),
                        source_ip: "127.0.0.1".to_string(),
                        device_type: "local".to_string(),
                        content: MessageContent::Text(message.clone().content),
                        created_at: message.created_at,
                    },
                    created_at: Utc::now(),
                };
                match channel_manager.send(save_message).await {
                    Ok(result) => match result {
                        Ok(_) => {
                            debug!("ChatAgent响应保存到数据库成功");
                        }
                        Err(e) => {
                            warn!("ChatAgent响应保存到数据库失败: {}", e);
                        }
                    },
                    Err(e) => {
                        error!("发送保存ChatAgent响应到ChannelManager失败: {}", e);
                    }
                }
            }

            let mut message_list = vec![];

            for chat_message in agent_response.content {
                message_list.push(serde_json::json!({
                    "sender": ai_name,
                    "content": chat_message.content,
                    "created_at":chat_message.created_at,
                }));
            };
            Ok(HttpResponse::Ok().json(message_list))
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
    agent_memory_hactor: web::Data<Addr<AgentMemoryHActor>>,
    user: web::ReqData<String>,
) -> ActixResult<HttpResponse> {
    // 获取用户名字符串
    let username = user.into_inner();
    // 调用 Actor
    let ai_name = match agent_memory_hactor.send(GetMyName {}).await {
        Ok(name) => name,
        Err(e) => {
            error!("获取AI名称失败: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() })));
        }
    };
    let result = channel_manager
        .send(GetMessages {
            user: username, // 使用中间件解析出的用户名
            ai_name,
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
                        "sender": m.user,
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
