use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

/// 用户发消息过来的消息结构体 - 兼容各种格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// 标识
    pub sender: String,
    /// 消息来源(ip地址)
    pub source_ip: String,
    /// 设备类型
    pub device_type: String,
    /// 消息内容
    pub content: MessageContent,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl UserMessage {
    pub fn new() -> Self {
        Self {
            sender: "unknown".to_string(),
            source_ip: "localhost".to_string(),
            device_type: "local".to_string(),
            content: MessageContent::Text("".to_string()),
            created_at: Utc::now(),
        }
    }

    // 修改发送者
    pub fn with_sender(mut self, sender: String) -> Self {
        self.sender = sender;
        self
    }

    // 修改消息
    pub fn with_content(mut self, content: MessageContent) -> Self {
        self.content = content;
        self
    }
}

// 1. 定义消息内容的枚举：要么是纯文本，要么是多模态部件数组
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Multimodal(Vec<ContentPart>),
}

impl MessageContent {
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

// 2. 定义多模态的具体部件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    InputAudio { input_audio: InputAudio },
}

// 3. 具体的图片结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// 4. 具体的音频结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAudio {
    pub data: String,
    pub format: String,
}
