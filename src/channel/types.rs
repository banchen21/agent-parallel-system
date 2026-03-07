use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 消息来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageSource {
    /// HTTP接口
    Api,
    /// 终端命令行
    Terminal,
    /// 内部系统消息
    Internal,
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// 聊天消息
    Chat,
    /// 任务指令
    Task,
    /// 系统命令
    System,
    /// 查询请求
    Query,
    /// 响应消息
    Response,
}

/// 消息优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessagePriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// 核心消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息唯一标识
    pub id: Uuid,
    /// 消息来源
    pub source: MessageSource,
    /// 消息类型
    pub message_type: MessageType,
    /// 消息优先级
    pub priority: MessagePriority,
    /// 发送者标识
    pub sender: String,
    /// 接收者标识（可选，为空则广播）
    pub recipient: Option<String>,
    /// 消息内容
    pub content: String,
    /// 附加元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间（可选）
    pub expires_at: Option<DateTime<Utc>>,
}

impl Message {
    /// 创建新消息
    pub fn new(
        source: MessageSource,
        message_type: MessageType,
        sender: String,
        content: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            message_type,
            priority: MessagePriority::Normal,
            sender,
            recipient: None,
            content,
            metadata: HashMap::new(),
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    /// 设置接收者
    pub fn with_recipient(mut self, recipient: String) -> Self {
        self.recipient = Some(recipient);
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// 设置过期时间
    pub fn with_expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// 检查消息是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }
}

/// 消息处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResult {
    /// 原消息ID
    pub message_id: Uuid,
    /// 处理状态
    pub status: ProcessingStatus,
    /// 处理结果内容
    pub content: Option<String>,
    /// 处理时间戳
    pub processed_at: DateTime<Utc>,
    /// 错误信息（如果有）
    pub error: Option<String>,
}

/// 处理状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessingStatus {
    /// 成功处理
    Success,
    /// 处理失败
    Failed,
    /// 处理中
    Processing,
    /// 已排队
    Queued,
    /// 已拒绝
    Rejected,
}

impl MessageResult {
    /// 创建成功结果
    pub fn success(message_id: Uuid, content: String) -> Self {
        Self {
            message_id,
            status: ProcessingStatus::Success,
            content: Some(content),
            processed_at: Utc::now(),
            error: None,
        }
    }

    /// 创建失败结果
    pub fn failed(message_id: Uuid, error: String) -> Self {
        Self {
            message_id,
            status: ProcessingStatus::Failed,
            content: None,
            processed_at: Utc::now(),
            error: Some(error),
        }
    }

    /// 创建处理中状态
    pub fn processing(message_id: Uuid) -> Self {
        Self {
            message_id,
            status: ProcessingStatus::Processing,
            content: None,
            processed_at: Utc::now(),
            error: None,
        }
    }
}

/// 通道层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// 最大队列大小
    pub max_queue_size: usize,
    /// 工作线程数量
    pub worker_threads: usize,
    /// 消息超时时间（秒）
    pub message_timeout_seconds: u64,
    /// 是否启用消息持久化
    pub enable_persistence: bool,
    /// 重试次数
    pub max_retries: u32,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            worker_threads: 4,
            message_timeout_seconds: 300, // 5分钟
            enable_persistence: false,
            max_retries: 3,
        }
    }
}