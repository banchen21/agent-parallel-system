use crate::channel::types::{Message, MessageResult, ChannelConfig, MessagePriority};
use crate::channel::handler::{MessageHandler, HandlerRegistry};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn, error};
use std::collections::HashMap;

/// 消息队列项
#[derive(Debug)]
struct QueueItem {
    message: Message,
    retry_count: u32,
}

/// 通道层管理器
pub struct ChannelManager {
    /// 配置
    config: ChannelConfig,
    /// 消息队列（按优先级分组）
    queues: Arc<RwLock<HashMap<MessagePriority, mpsc::UnboundedSender<QueueItem>>>>,
    /// 处理器注册表
    handler_registry: Arc<HandlerRegistry>,
    /// 消息结果广播通道
    result_sender: broadcast::Sender<MessageResult>,
    /// 运行状态
    is_running: Arc<RwLock<bool>>,
    /// 统计信息
    stats: Arc<RwLock<ChannelStats>>,
}

/// 通道统计信息
#[derive(Debug, Default, Clone)]
pub struct ChannelStats {
    /// 已处理消息总数
    pub total_processed: u64,
    /// 成功处理数
    pub successful: u64,
    /// 失败处理数
    pub failed: u64,
    /// 当前队列大小
    pub queue_size: usize,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
}

impl ChannelManager {
    /// 创建新的通道管理器
    pub fn new(config: ChannelConfig) -> Self {
        let (result_sender, _) = broadcast::channel(1000);
        
        Self {
            config,
            queues: Arc::new(RwLock::new(HashMap::new())),
            handler_registry: Arc::new(HandlerRegistry::new()),
            result_sender,
            is_running: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(ChannelStats::default())),
        }
    }
    
    /// 启动通道管理器
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("通道管理器已经在运行中");
            return Ok(());
        }
        
        *is_running = true;
        info!("启动通道管理器，工作线程数: {}", self.config.worker_threads);
        
        // 初始化优先级队列
        self.init_queues().await?;
        
        // 启动工作线程
        for i in 0..self.config.worker_threads {
            self.start_worker(i).await?;
        }
        
        // 启动清理任务
        self.start_cleanup_task().await;
        
        info!("通道管理器启动成功");
        Ok(())
    }
    
    /// 停止通道管理器
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        info!("通道管理器已停止");
        Ok(())
    }
    
    /// 发送消息
    pub async fn send_message(&self, message: Message) -> Result<()> {
        let message_id = message.id;
        
        if message.is_expired() {
            warn!("消息已过期，拒绝处理: {}", message_id);
            return Ok(());
        }
        
        let queues = self.queues.read().await;
        if let Some(sender) = queues.get(&message.priority) {
            let queue_item = QueueItem {
                message,
                retry_count: 0,
            };
            
            if let Err(_) = sender.send(queue_item) {
                error!("消息队列已满，丢弃消息");
                return Err(anyhow::anyhow!("消息队列已满"));
            }
            
            debug!("消息已加入队列: {}", message_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("未找到对应优先级的队列"))
        }
    }
    
    /// 注册消息处理器
    pub async fn register_handler(&self, handler: Arc<dyn MessageHandler>) {
        self.handler_registry.register(handler).await;
        info!("消息处理器注册成功");
    }
    
    /// 获取结果接收器
    pub fn subscribe_results(&self) -> broadcast::Receiver<MessageResult> {
        self.result_sender.subscribe()
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> ChannelStats {
        self.stats.read().await.clone()
    }
    
    /// 初始化优先级队列
    async fn init_queues(&self) -> Result<()> {
        let mut queues = self.queues.write().await;
        
        // 为每个优先级创建队列
        for priority in [
            MessagePriority::Critical,
            MessagePriority::High,
            MessagePriority::Normal,
            MessagePriority::Low,
        ] {
            let (sender, receiver) = mpsc::unbounded_channel();
            queues.insert(priority.clone(), sender);
            
            // 启动队列处理任务
            self.start_queue_processor(priority, receiver).await;
        }
        
        Ok(())
    }
    
    /// 启动队列处理器
    async fn start_queue_processor(
        &self,
        priority: MessagePriority,
        mut receiver: mpsc::UnboundedReceiver<QueueItem>,
    ) {
        let handler_registry = Arc::clone(&self.handler_registry);
        let result_sender = self.result_sender.clone();
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            while *is_running.read().await {
                match receiver.recv().await {
                    Some(queue_item) => {
                        let start_time = std::time::Instant::now();
                        
                        // 查找合适的处理器
                        let handlers = handler_registry.find_handlers(&queue_item.message).await;
                        
                        if handlers.is_empty() {
                            warn!("未找到能处理消息的处理器: {}", queue_item.message.id);
                            let result = MessageResult::failed(
                                queue_item.message.id,
                                "未找到合适的处理器".to_string(),
                            );
                            let _ = result_sender.send(result);
                            continue;
                        }
                        
                        // 使用第一个匹配的处理器处理消息
                        let handler = &handlers[0];
                        let message = queue_item.message;
                        let message_id = message.id;
                        
                        match timeout(
                            Duration::from_secs(config.message_timeout_seconds),
                            handler.handle(message),
                        ).await {
                            Ok(Ok(result)) => {
                                let processing_time = start_time.elapsed().as_millis() as f64;
                                
                                // 更新统计信息
                                {
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.total_processed += 1;
                                    stats_guard.successful += 1;
                                    stats_guard.avg_processing_time_ms = 
                                        (stats_guard.avg_processing_time_ms * (stats_guard.total_processed - 1) as f64 + processing_time) 
                                        / stats_guard.total_processed as f64;
                                }
                                
                                debug!("消息处理成功: {}, 耗时: {}ms", message_id, processing_time);
                                let _ = result_sender.send(result);
                            }
                            Ok(Err(e)) => {
                                error!("消息处理失败: {}, 错误: {}", message_id, e);
                                let result = MessageResult::failed(message_id, e.to_string());
                                let _ = result_sender.send(result);
                                
                                // 更新统计信息
                                {
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.total_processed += 1;
                                    stats_guard.failed += 1;
                                }
                            }
                            Err(_) => {
                                error!("消息处理超时: {}", message_id);
                                let result = MessageResult::failed(message_id, "处理超时".to_string());
                                let _ = result_sender.send(result);
                                
                                // 更新统计信息
                                {
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.total_processed += 1;
                                    stats_guard.failed += 1;
                                }
                            }
                        }
                    }
                    None => {
                        // 通道关闭，退出循环
                        break;
                    }
                }
            }
        });
    }
    
    /// 启动工作线程
    async fn start_worker(&self, worker_id: usize) -> Result<()> {
        info!("启动工作线程: {}", worker_id);
        
        // 这里可以添加更多的工作线程逻辑
        // 目前主要工作由队列处理器完成
        
        Ok(())
    }
    
    /// 启动清理任务
    async fn start_cleanup_task(&self) {
        let stats = Arc::clone(&self.stats);
        let queues = Arc::clone(&self.queues);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // 更新队列大小统计
                let mut total_queue_size = 0;
                let queues_guard = queues.read().await;
                for sender in queues_guard.values() {
                    // 注意：这里无法直接获取队列大小，实际实现中可能需要使用其他方式
                    total_queue_size += 0; // 占位符
                }
                
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.queue_size = total_queue_size;
                }
                
                debug!("清理任务完成，当前队列大小: {}", total_queue_size);
            }
        });
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new(ChannelConfig::default())
    }
}