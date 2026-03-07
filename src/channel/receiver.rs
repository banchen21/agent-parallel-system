use crate::channel::types::{Message, MessageSource, MessageType, MessagePriority};
use crate::channel::manager::ChannelManager;
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, error};
use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API消息接收器
pub struct ApiReceiver {
    channel_manager: Arc<ChannelManager>,
}

impl ApiReceiver {
    /// 创建新的API接收器
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self { channel_manager }
    }
    
    /// 处理HTTP请求
    pub async fn handle_request(
        &self,
        req: HttpRequest,
        payload: web::Json<ApiMessage>,
    ) -> ActixResult<HttpResponse> {
        debug!("收到API请求: {:?}", payload);
        
        // 将API消息转换为内部消息
        match self.convert_api_message(payload.into_inner(), req).await {
            Ok(message) => {
                // 发送到通道管理器
                if let Err(e) = self.channel_manager.send_message(message).await {
                    error!("发送消息到通道失败: {}", e);
                    return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "消息发送失败",
                        "details": e.to_string()
                    })));
                }
                
                Ok(HttpResponse::Accepted().json(serde_json::json!({
                    "status": "accepted",
                    "message": "消息已接收并正在处理"
                })))
            }
            Err(e) => {
                error!("转换API消息失败: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "消息格式错误",
                    "details": e.to_string()
                })))
            }
        }
    }
    
    /// 转换API消息为内部消息
    async fn convert_api_message(&self, api_msg: ApiMessage, req: HttpRequest) -> Result<Message> {
        let message_type = match api_msg.message_type.as_str() {
            "chat" => MessageType::Chat,
            "task" => MessageType::Task,
            "system" => MessageType::System,
            "query" => MessageType::Query,
            _ => MessageType::Chat, // 默认为聊天消息
        };
        
        let priority = match api_msg.priority.as_str() {
            "low" => MessagePriority::Low,
            "normal" => MessagePriority::Normal,
            "high" => MessagePriority::High,
            "critical" => MessagePriority::Critical,
            _ => MessagePriority::Normal,
        };
        
        // 从请求头获取客户端信息
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        
        let mut message = Message::new(
            MessageSource::Api,
            message_type,
            api_msg.sender.unwrap_or_else(|| client_ip),
            api_msg.content,
        )
        .with_priority(priority);
        
        // 设置接收者
        if let Some(recipient) = api_msg.recipient {
            message = message.with_recipient(recipient);
        }
        
        // 添加元数据
        if let Some(metadata) = api_msg.metadata {
            for (key, value) in metadata {
                message = message.with_metadata(key, value);
            }
        }
        
        // 添加API特有的元数据
        message = message.with_metadata(
            "user_agent".to_string(),
            serde_json::Value::String(
                req.headers()
                    .get("user-agent")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string()
            ),
        );
        
        message = message.with_metadata(
            "endpoint".to_string(),
            serde_json::Value::String(req.path().to_string()),
        );
        
        Ok(message)
    }
}

/// API消息格式
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiMessage {
    /// 消息内容
    pub content: String,
    /// 消息类型
    #[serde(default = "default_message_type")]
    pub message_type: String,
    /// 优先级
    #[serde(default = "default_priority")]
    pub priority: String,
    /// 发送者（可选）
    pub sender: Option<String>,
    /// 接收者（可选）
    pub recipient: Option<String>,
    /// 元数据（可选）
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

fn default_message_type() -> String {
    "chat".to_string()
}

fn default_priority() -> String {
    "normal".to_string()
}

/// 终端消息接收器
pub struct TerminalReceiver {
    channel_manager: Arc<ChannelManager>,
}

impl TerminalReceiver {
    /// 创建新的终端接收器
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self { channel_manager }
    }
    
    /// 处理终端输入
    pub async fn handle_input(&self, input: String, sender: String) -> Result<()> {
        debug!("收到终端输入: {}", input);
        
        // 解析终端命令
        let message = self.parse_terminal_input(input, sender).await?;
        
        // 发送到通道管理器
        self.channel_manager.send_message(message).await?;
        
        Ok(())
    }
    
    /// 解析终端输入
    async fn parse_terminal_input(&self, input: String, sender: String) -> Result<Message> {
        let trimmed = input.trim();
        
        // 检查是否为系统命令
        if trimmed.starts_with('/') {
            let command = &trimmed[1..];
            let parts: Vec<&str> = command.split_whitespace().collect();
            
            if parts.is_empty() {
                return Err(anyhow::anyhow!("无效的命令"));
            }
            
            let message_type = MessageType::System;
            let priority = if parts[0] == "shutdown" || parts[0] == "emergency" {
                MessagePriority::Critical
            } else {
                MessagePriority::High
            };
            
            let mut message = Message::new(
                MessageSource::Terminal,
                message_type,
                sender,
                input.clone(),
            )
            .with_priority(priority);
            
            // 添加命令元数据
            message = message.with_metadata(
                "command".to_string(),
                serde_json::Value::String(parts[0].to_string()),
            );
            
            if parts.len() > 1 {
                message = message.with_metadata(
                    "args".to_string(),
                    serde_json::Value::Array(
                        parts[1..].iter()
                            .map(|arg| serde_json::Value::String(arg.to_string()))
                            .collect()
                    ),
                );
            }
            
            Ok(message)
        } else {
            // 普通聊天消息
            let message_type = if trimmed.contains("任务") || trimmed.contains("执行") {
                MessageType::Task
            } else if trimmed.contains("?") || trimmed.contains("查询") {
                MessageType::Query
            } else {
                MessageType::Chat
            };
            
            Ok(Message::new(
                MessageSource::Terminal,
                message_type,
                sender,
                input,
            ))
        }
    }
    
    /// 启动交互式终端模式
    pub async fn start_interactive_mode(&self) -> Result<()> {
        info!("终端接收器已准备就绪，可通过API接口发送消息");
        // 暂时简化实现，避免Send trait问题
        // 实际的终端交互可以通过API接口实现
        Ok(())
    }
}

/// 消息接收器工厂
pub struct ReceiverFactory;

impl ReceiverFactory {
    /// 创建API接收器
    pub fn create_api_receiver(channel_manager: Arc<ChannelManager>) -> Arc<ApiReceiver> {
        Arc::new(ApiReceiver::new(channel_manager))
    }
    
    /// 创建终端接收器
    pub fn create_terminal_receiver(channel_manager: Arc<ChannelManager>) -> Arc<TerminalReceiver> {
        Arc::new(TerminalReceiver::new(channel_manager))
    }
}

/// 配置API路由
pub fn configure_api_routes(cfg: &mut web::ServiceConfig, api_receiver: Arc<ApiReceiver>) {
    cfg.service(
        web::resource("/message")
            .route(web::post().to(handle_message))
    )
    .service(
        web::resource("/chat")
            .route(web::post().to(handle_chat))
    )
    .service(
        web::resource("/task")
            .route(web::post().to(handle_task))
    )
    .service(
        web::resource("/system")
            .route(web::post().to(handle_system))
    );
}

async fn handle_message(
    req: HttpRequest,
    payload: web::Json<ApiMessage>,
    api_receiver: web::Data<Arc<ApiReceiver>>,
) -> ActixResult<HttpResponse> {
    api_receiver.handle_request(req, payload).await
}

async fn handle_chat(
    req: HttpRequest,
    payload: web::Json<ApiMessage>,
    api_receiver: web::Data<Arc<ApiReceiver>>,
) -> ActixResult<HttpResponse> {
    // 自动设置为聊天消息
    let mut api_msg = payload.into_inner();
    api_msg.message_type = "chat".to_string();
    api_receiver.handle_request(req, web::Json(api_msg)).await
}

async fn handle_task(
    req: HttpRequest,
    payload: web::Json<ApiMessage>,
    api_receiver: web::Data<Arc<ApiReceiver>>,
) -> ActixResult<HttpResponse> {
    // 自动设置为任务消息
    let mut api_msg = payload.into_inner();
    api_msg.message_type = "task".to_string();
    api_receiver.handle_request(req, web::Json(api_msg)).await
}

async fn handle_system(
    req: HttpRequest,
    payload: web::Json<ApiMessage>,
    api_receiver: web::Data<Arc<ApiReceiver>>,
) -> ActixResult<HttpResponse> {
    // 自动设置为系统消息
    let mut api_msg = payload.into_inner();
    api_msg.message_type = "system".to_string();
    api_msg.priority = "high".to_string();
    api_receiver.handle_request(req, web::Json(api_msg)).await
}