//! 通道层模块
//! 
//! 该模块负责接收来自接口和终端的消息，并进行统一处理和分发。
//! 支持优先级队列、多种消息类型和处理器注册机制。

pub mod types;
pub mod handler;
pub mod manager;
pub mod receiver;
pub mod persistence;

// 重新导出主要类型
pub use types::*;
pub use handler::{MessageHandler, HandlerRegistry, ChatHandler, TaskHandler, SystemHandler};
pub use manager::ChannelManager;
pub use receiver::{ApiReceiver, TerminalReceiver, ReceiverFactory, configure_api_routes, ApiMessage};
pub use persistence::{MessagePersistence, MessageStats};

/// 通道层错误类型
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("消息队列已满")]
    QueueFull,
    
    #[error("消息已过期")]
    MessageExpired,
    
    #[error("未找到合适的处理器")]
    NoHandlerFound,
    
    #[error("处理超时")]
    ProcessingTimeout,
    
    #[error("通道未启动")]
    ChannelNotStarted,
    
    #[error("无效的消息格式: {0}")]
    InvalidMessageFormat(String),
    
    #[error("内部错误: {0}")]
    InternalError(String),
}

// 移除冲突的From实现，anyhow已经提供了通用的From实现

/// 通道层构建器
pub struct ChannelBuilder {
    config: ChannelConfig,
}

impl ChannelBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: ChannelConfig::default(),
        }
    }
    
    /// 设置最大队列大小
    pub fn with_max_queue_size(mut self, size: usize) -> Self {
        self.config.max_queue_size = size;
        self
    }
    
    /// 设置工作线程数量
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.config.worker_threads = threads;
        self
    }
    
    /// 设置消息超时时间
    pub fn with_message_timeout(mut self, timeout_seconds: u64) -> Self {
        self.config.message_timeout_seconds = timeout_seconds;
        self
    }
    
    /// 启用消息持久化
    pub fn with_persistence(mut self, enabled: bool) -> Self {
        self.config.enable_persistence = enabled;
        self
    }
    
    /// 设置最大重试次数
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }
    
    /// 构建通道管理器
    pub fn build(self) -> ChannelManager {
        ChannelManager::new(self.config)
    }
}

impl Default for ChannelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_message_creation() {
        let message = Message::new(
            MessageSource::Api,
            MessageType::Chat,
            "test_user".to_string(),
            "Hello, World!".to_string(),
        )
        .with_priority(MessagePriority::High)
        .with_recipient("bot".to_string());
        
        assert_eq!(message.source, MessageSource::Api);
        assert_eq!(message.message_type, MessageType::Chat);
        assert_eq!(message.priority, MessagePriority::High);
        assert_eq!(message.sender, "test_user");
        assert_eq!(message.recipient, Some("bot".to_string()));
        assert_eq!(message.content, "Hello, World!");
    }
    
    #[tokio::test]
    async fn test_channel_builder() {
        let channel = ChannelBuilder::new()
            .with_max_queue_size(5000)
            .with_worker_threads(8)
            .with_message_timeout(600)
            .with_persistence(true)
            .with_max_retries(5)
            .build();
            
        let stats = channel.get_stats().await;
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
    }
    
    #[tokio::test]
    async fn test_message_result() {
        let message_id = uuid::Uuid::new_v4();
        
        let success_result = MessageResult::success(message_id, "处理成功".to_string());
        assert_eq!(success_result.message_id, message_id);
        assert_eq!(success_result.status, ProcessingStatus::Success);
        assert_eq!(success_result.content, Some("处理成功".to_string()));
        assert!(success_result.error.is_none());
        
        let failed_result = MessageResult::failed(message_id, "处理失败".to_string());
        assert_eq!(failed_result.message_id, message_id);
        assert_eq!(failed_result.status, ProcessingStatus::Failed);
        assert!(failed_result.content.is_none());
        assert_eq!(failed_result.error, Some("处理失败".to_string()));
    }
}