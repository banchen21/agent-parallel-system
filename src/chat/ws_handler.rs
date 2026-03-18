/// WebSocket 聊天处理器
///
/// 客户端升级协议后，通过 WS 长连接与 ChatAgent 交互：
///   客户端 → 服务端：`{ "content": "...", "device_type": "web" }`
///   服务端 → 客户端：`{ "type": "thinking" }` / `{ "type": "message", "sender": "...", "content": "...", "created_at": "..." }` / `{ "type": "error", "message": "..." }`
///
/// 认证：通过 URL 查询参数 `?token=<JWT AccessToken>`
use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, get, web};
use actix_web_actors::ws;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::api::auth_utils::validate_token;
use crate::channel::actor_messages::{
    ChannelEvent, ChannelManagerActor, SaveMessage, SubscribeChannelNotify, UnsubscribeChannelNotify,
};
use crate::chat::chat_agent::{ChatAgent, OtherUserMessage};
use crate::chat::model::{MessageContent, UserMessage};
use crate::graph_memory::actor_memory::{AgentMemoryActor, GetMyName};

// ─── 协议结构 ─────────────────────────────────────────────────────

/// 客户端 → 服务端（文本帧 JSON）
#[derive(Debug, Deserialize)]
struct WsIncoming {
    content: String,
    #[serde(default = "default_device")]
    device_type: String,
}

fn default_device() -> String {
    "web".to_string()
}

/// 服务端 → 客户端（文本帧 JSON）
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsOutgoing {
    /// AI 正在思考中（收到用户消息后立即推送）
    Thinking,
    /// AI 回复消息
    Message {
        sender: String,
        content: String,
        created_at: String,
    },
    /// 任务进度/完成通知
    TaskProgress {
        sender: String,
        content: String,
        created_at: String,
    },
    /// 错误通知
    Error { message: String },
}

// ─── WS Session Actor ─────────────────────────────────────────────

pub struct ChatWsSession {
    session_id: String,
    username: String,
    client_ip: String,
    chat_agent: Addr<ChatAgent>,
    channel_manager: Addr<ChannelManagerActor>,
    agent_memory: Addr<AgentMemoryActor>,
}

impl Actor for ChatWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.channel_manager.do_send(SubscribeChannelNotify {
            session_id: self.session_id.clone(),
            recipient: ctx.address().recipient(),
        });

        info!(user = %self.username, "WS 聊天会话已建立");
        // 每 30 秒发送一次 ping 保持长连接
        ctx.run_interval(std::time::Duration::from_secs(30), |_, ctx| {
            ctx.ping(b"");
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.channel_manager.do_send(UnsubscribeChannelNotify {
            session_id: self.session_id.clone(),
        });
        info!(user = %self.username, "WS 聊天会话已关闭");
    }
}

/// 内部消息：将推送文本投递到 WS context（用于跨 spawn 边界推送）
#[derive(Message)]
#[rtype(result = "()")]
struct PushText(String);

impl Handler<PushText> for ChatWsSession {
    type Result = ();

    fn handle(&mut self, msg: PushText, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.text(msg.0);
    }
}

impl Handler<ChannelEvent> for ChatWsSession {
    type Result = ();

    fn handle(&mut self, msg: ChannelEvent, ctx: &mut ws::WebsocketContext<Self>) {
        if msg.user != "任务系统" {
            return;
        }

        let out = WsOutgoing::TaskProgress {
            sender: "任务系统".to_string(),
            content: msg.content,
            created_at: msg.created_at.to_rfc3339(),
        };
        if let Ok(json) = serde_json::to_string(&out) {
            ctx.text(json);
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ChatWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // 解析客户端消息
                let incoming: WsIncoming = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => {
                        let err = serde_json::to_string(&WsOutgoing::Error {
                            message: "消息格式错误，请发送 JSON：{\"content\":\"...\"}".to_string(),
                        })
                        .unwrap_or_default();
                        ctx.text(err);
                        return;
                    }
                };

                // 立即推送"思考中"指示，让前端显示加载状态
                if let Ok(json) = serde_json::to_string(&WsOutgoing::Thinking) {
                    ctx.text(json);
                }

                let user_message = UserMessage {
                    sender: self.username.clone(),
                    source_ip: self.client_ip.clone(),
                    device_type: incoming.device_type,
                    content: MessageContent::Text(incoming.content),
                    created_at: Utc::now(),
                };

                let chat_agent = self.chat_agent.clone();
                let channel_manager = self.channel_manager.clone();
                let agent_memory = self.agent_memory.clone();
                // 获取 actor 地址，用于在 spawn 中回推消息
                let addr = ctx.address();
                let user_msg = user_message.clone();

                tokio::spawn(async move {
                    // 1. 获取 AI 名称
                    let ai_name = match agent_memory.send(GetMyName {}).await {
                        Ok(name) => name,
                        Err(e) => {
                            error!("获取 AI 名称失败: {}", e);
                            "AI".to_string()
                        }
                    };

                    // 2. 持久化用户消息
                    if let Err(e) = channel_manager
                        .send(SaveMessage {
                            message: user_msg.clone(),
                        })
                        .await
                    {
                        error!("保存用户消息失败: {}", e);
                    }

                    // 3. 调用 ChatAgent 推理
                    match chat_agent
                        .send(OtherUserMessage { content: user_msg })
                        .await
                    {
                        Ok(Ok(resp)) => {
                            for chat_msg in resp.content {
                                // 4. 持久化 AI 消息
                                if let Err(e) = channel_manager
                                    .send(SaveMessage {
                                        message: UserMessage {
                                            sender: ai_name.clone(),
                                            source_ip: "127.0.0.1".to_string(),
                                            device_type: "local".to_string(),
                                            content: MessageContent::Text(chat_msg.content.clone()),
                                            created_at: Utc::now(),
                                        },
                                    })
                                    .await
                                {
                                    error!("保存 AI 消息失败: {}", e);
                                }

                                // 5. 推送给客户端
                                let out = WsOutgoing::Message {
                                    sender: ai_name.clone(),
                                    content: chat_msg.content,
                                    created_at: chat_msg.created_at.to_rfc3339(),
                                };
                                if let Ok(json) = serde_json::to_string(&out) {
                                    addr.do_send(PushText(json));
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            let json = serde_json::to_string(&WsOutgoing::Error {
                                message: e.to_string(),
                            })
                            .unwrap_or_default();
                            addr.do_send(PushText(json));
                        }
                        Err(e) => {
                            error!("ChatAgent 通信失败: {}", e);
                            let json = serde_json::to_string(&WsOutgoing::Error {
                                message: "服务器内部错误".to_string(),
                            })
                            .unwrap_or_default();
                            addr.do_send(PushText(json));
                        }
                    }
                });
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

// ─── HTTP → WS 升级端点 ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

/// GET /ws/chat?token=<JWT>
///
/// 将 HTTP 连接升级为 WebSocket，认证通过后创建 ChatWsSession。
/// 接受的 token 为前端 localStorage 中的 access_token。
#[get("/ws/chat")]
pub async fn ws_chat_handler(
    req: HttpRequest,
    stream: web::Payload,
    query: web::Query<WsQuery>,
    chat_agent: web::Data<Addr<ChatAgent>>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    agent_memory: web::Data<Addr<AgentMemoryActor>>,
) -> ActixResult<HttpResponse> {
    // 验证 JWT
    let username = match validate_token(&query.token) {
        Ok(claims) => claims.sub,
        Err(_) => return Ok(HttpResponse::Unauthorized().body("无效的 token")),
    };

    let client_ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let session = ChatWsSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        username,
        client_ip,
        chat_agent: chat_agent.get_ref().clone(),
        channel_manager: channel_manager.get_ref().clone(),
        agent_memory: agent_memory.get_ref().clone(),
    };

    ws::start(session, &req, stream)
}
