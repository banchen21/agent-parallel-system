use async_trait::async_trait;
use crate::channel::types::{Message, MessageResult};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 消息处理器接口
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// 获取处理器名称
    fn name(&self) -> &str;
    
    /// 检查是否可以处理该消息
    async fn can_handle(&self, message: &Message) -> bool;
    
    /// 处理消息
    async fn handle(&self, message: Message) -> Result<MessageResult>;
    
    /// 获取处理器优先级（数值越小优先级越高）
    fn priority(&self) -> u32 {
        100
    }
    
    /// 处理器是否支持并发处理
    fn supports_concurrent(&self) -> bool {
        true
    }
}

/// 处理器注册表
pub struct HandlerRegistry {
    handlers: Arc<RwLock<Vec<Arc<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 使用block来异步获取长度，但Debug trait不支持async，所以简化实现
        f.debug_struct("HandlerRegistry")
            .field("handlers", &"<Arc<RwLock<Vec<MessageHandler>>>>")
            .finish()
    }
}

impl HandlerRegistry {
    /// 创建新的处理器注册表
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// 注册处理器
    pub async fn register(&self, handler: Arc<dyn MessageHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        // 按优先级排序
        handlers.sort_by_key(|h| h.priority());
    }
    
    /// 注销处理器
    pub async fn unregister(&self, handler_name: &str) -> bool {
        let mut handlers = self.handlers.write().await;
        let initial_len = handlers.len();
        handlers.retain(|h| h.name() != handler_name);
        handlers.len() != initial_len
    }
    
    /// 查找能处理该消息的处理器
    pub async fn find_handlers(&self, message: &Message) -> Vec<Arc<dyn MessageHandler>> {
        let handlers = self.handlers.read().await;
        let mut matching_handlers = Vec::new();
        
        for handler in handlers.iter() {
            if handler.can_handle(message).await {
                matching_handlers.push(Arc::clone(handler));
            }
        }
        
        matching_handlers
    }
    
    /// 获取所有已注册的处理器名称
    pub async fn list_handlers(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.iter().map(|h| h.name().to_string()).collect()
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 基础消息处理器实现
pub struct BaseHandler {
    name: String,
    priority: u32,
    supported_types: Vec<crate::channel::types::MessageType>,
    supported_sources: Vec<crate::channel::types::MessageSource>,
}

impl BaseHandler {
    /// 创建新的基础处理器
    pub fn new(
        name: String,
        priority: u32,
        supported_types: Vec<crate::channel::types::MessageType>,
        supported_sources: Vec<crate::channel::types::MessageSource>,
    ) -> Self {
        Self {
            name,
            priority,
            supported_types,
            supported_sources,
        }
    }
}

#[async_trait]
impl MessageHandler for BaseHandler {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn can_handle(&self, message: &Message) -> bool {
        let type_supported = self.supported_types.contains(&message.message_type);
        let source_supported = self.supported_sources.contains(&message.source);
        type_supported && source_supported
    }
    
    async fn handle(&self, message: Message) -> Result<MessageResult> {
        // 默认实现：简单回显
        Ok(MessageResult::success(
            message.id,
            format!("Handler {} processed message: {}", self.name, message.content),
        ))
    }
    
    fn priority(&self) -> u32 {
        self.priority
    }
}

/// 聊天消息处理器
pub struct ChatHandler {
    base: BaseHandler,
}

impl ChatHandler {
    pub fn new() -> Self {
        Self {
            base: BaseHandler::new(
                "ChatHandler".to_string(),
                10, // 高优先级
                vec![crate::channel::types::MessageType::Chat],
                vec![
                    crate::channel::types::MessageSource::Api,
                    crate::channel::types::MessageSource::Terminal,
                ],
            ),
        }
    }
}

#[async_trait]
impl MessageHandler for ChatHandler {
    fn name(&self) -> &str {
        self.base.name()
    }
    
    async fn can_handle(&self, message: &Message) -> bool {
        self.base.can_handle(message).await
    }
    
    async fn handle(&self, message: Message) -> Result<MessageResult> {
        // 这里可以集成聊天逻辑，包括人格识别、图数据库查询等
        let response = format!("聊天回复: {}", message.content);
        Ok(MessageResult::success(message.id, response))
    }
    
    fn priority(&self) -> u32 {
        self.base.priority()
    }
}

/// 任务处理器
pub struct TaskHandler {
    base: BaseHandler,
}

impl TaskHandler {
    pub fn new() -> Self {
        Self {
            base: BaseHandler::new(
                "TaskHandler".to_string(),
                20, // 中等优先级
                vec![crate::channel::types::MessageType::Task],
                vec![
                    crate::channel::types::MessageSource::Api,
                    crate::channel::types::MessageSource::Terminal,
                ],
            ),
        }
    }
}

#[async_trait]
impl MessageHandler for TaskHandler {
    fn name(&self) -> &str {
        self.base.name()
    }
    
    async fn can_handle(&self, message: &Message) -> bool {
        self.base.can_handle(message).await
    }
    
    async fn handle(&self, message: Message) -> Result<MessageResult> {
        // 这里可以集成任务分配逻辑，包括工作空间管理等
        let response = format!("任务已接收并分配: {}", message.content);
        Ok(MessageResult::success(message.id, response))
    }
    
    fn priority(&self) -> u32 {
        self.base.priority()
    }
}

/// 系统消息处理器
pub struct SystemHandler {
    base: BaseHandler,
}

impl SystemHandler {
    pub fn new() -> Self {
        Self {
            base: BaseHandler::new(
                "SystemHandler".to_string(),
                1, // 最高优先级
                vec![crate::channel::types::MessageType::System],
                vec![
                    crate::channel::types::MessageSource::Api,
                    crate::channel::types::MessageSource::Terminal,
                    crate::channel::types::MessageSource::Internal,
                ],
            ),
        }
    }
}

#[async_trait]
impl MessageHandler for SystemHandler {
    fn name(&self) -> &str {
        self.base.name()
    }
    
    async fn can_handle(&self, message: &Message) -> bool {
        self.base.can_handle(message).await
    }
    
    async fn handle(&self, message: Message) -> Result<MessageResult> {
        // 处理系统级命令
        match message.content.as_str() {
            "status" => Ok(MessageResult::success(message.id, "系统运行正常".to_string())),
            "shutdown" => Ok(MessageResult::success(message.id, "系统关闭命令已接收".to_string())),
            _ => Ok(MessageResult::success(message.id, format!("系统命令已处理: {}", message.content))),
        }
    }
    
    fn priority(&self) -> u32 {
        self.base.priority()
    }
}