# UserMessage API 接口文档

## 概述

UserMessage 是系统中用于表示用户消息的核心数据结构，支持多种消息格式和内容类型。本文档详细说明了 UserMessage 的 JSON 格式和各种使用示例。

## UserMessage 结构

```json
{
  "user": "string",
  "source_ip": "string", 
  "content": "MessageContent",
  "message_type": "Chat|Task",
  "metadata": {},
  "created_at": "ISO8601 datetime"
}
```

### 字段说明

| 字段名 | 类型 | 必需 | 说明 |
|--------|------|------|------|
| user | string | 是 | 用户标识符 |
| source_ip | string | 是 | 消息来源IP地址 |
| content | MessageContent | 是 | 消息内容（支持多种格式） |
| message_type | MessageType | 否 | 消息类型，默认为 "Chat" |
| metadata | object | 否 | 附加元数据 |
| created_at | string | 是 | 创建时间（ISO8601格式） |

## MessageType 枚举

```json
"Chat" | "Task"
```

- **Chat**: 聊天消息
- **Task**: 任务消息

## MessageContent 类型

MessageContent 支持以下几种格式：

### 1. 纯文本 (Text)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": "你好，我想了解一下这个系统",
  "message_type": "Chat",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

### 2. 富文本 (RichText)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": {
    "text": "# 标题\n这是一个**粗体**文本",
    "format": "Markdown"
  },
  "message_type": "Chat",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

支持的格式类型：
- `Plain`: 纯文本
- `Markdown`: Markdown格式
- `Html`: HTML格式
- `Json`: JSON格式

### 3. 结构化消息 (Structured)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": {
    "message_type": "form_submission",
    "data": {
      "name": "张三",
      "email": "zhangsan@example.com",
      "age": 25,
      "interests": ["编程", "阅读", "音乐"]
    }
  },
  "message_type": "Chat",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

### 4. 多媒体消息 (Media)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": {
    "media_type": "Image",
    "url": "https://example.com/image.jpg",
    "caption": "这是一张图片",
    "metadata": {
      "width": 1920,
      "height": 1080,
      "size": "2.5MB"
    }
  },
  "message_type": "Chat",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

支持的媒体类型：
- `Image`: 图片
- `Video`: 视频
- `Audio`: 音频
- `Document`: 文档
- `Link`: 链接

### 5. 命令消息 (Command)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": {
    "command": "help",
    "args": ["list", "commands"]
  },
  "message_type": "Task",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

### 6. 文件消息 (File)

```json
{
  "user": "user123",
  "source_ip": "192.168.1.100",
  "content": {
    "filename": "document.pdf",
    "content_type": "application/pdf",
    "size": 1048576,
    "url": "https://example.com/files/document.pdf"
  },
  "message_type": "Chat",
  "metadata": {},
  "created_at": "2024-03-08T10:30:00Z"
}
```

## API 使用示例

### 发送纯文本消息

```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user": "user123",
    "source_ip": "192.168.1.100",
    "content": "你好，我想了解一下这个系统",
    "message_type": "Chat",
    "metadata": {},
    "created_at": "2024-03-08T10:30:00Z"
  }'
```

### 发送富文本消息

```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user": "user123",
    "source_ip": "192.168.1.100",
    "content": {
      "text": "# 标题\n这是一个**粗体**文本",
      "format": "Markdown"
    },
    "message_type": "Chat",
    "metadata": {},
    "created_at": "2024-03-08T10:30:00Z"
  }'
```

### 发送命令消息

```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user": "user123",
    "source_ip": "192.168.1.100",
    "content": {
      "command": "status",
      "args": ["system"]
    },
    "message_type": "Task",
    "metadata": {},
    "created_at": "2024-03-08T10:30:00Z"
  }'
```

### 发送带元数据的消息

```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user": "user123",
    "source_ip": "192.168.1.100",
    "content": "请帮我分析这个数据",
    "message_type": "Chat",
    "metadata": {
      "priority": "high",
      "category": "data_analysis",
      "session_id": "sess_12345"
    },
    "created_at": "2024-03-08T10:30:00Z"
  }'
```

## 响应格式

### 成功响应

```json
{
  "status": "success",
  "message_id": "uuid-string",
  "response": "AI助手的回复内容",
  "timestamp": "2024-03-08T10:30:05Z"
}
```

### 错误响应

```json
{
  "status": "error",
  "error_code": "INVALID_MESSAGE_FORMAT",
  "error_message": "消息格式不正确",
  "timestamp": "2024-03-08T10:30:05Z"
}
```

## 错误代码

| 错误代码 | 说明 |
|----------|------|
| INVALID_MESSAGE_FORMAT | 消息格式不正确 |
| MISSING_REQUIRED_FIELD | 缺少必需字段 |
| INVALID_CONTENT_TYPE | 无效的内容类型 |
| MESSAGE_TOO_LARGE | 消息过大 |
| RATE_LIMIT_EXCEEDED | 请求频率超限 |

## 注意事项

1. **时间格式**: `created_at` 字段必须使用 ISO8601 格式
2. **内容类型**: `content` 字段根据不同的消息类型有不同的结构
3. **元数据**: `metadata` 字段可以包含任意键值对，用于传递额外信息
4. **消息大小**: 建议消息内容不超过 1MB
5. **IP地址**: `source_ip` 字段用于记录消息来源，支持 IPv4 和 IPv6 格式

## 验证规则

- `user`: 长度 1-255 字符
- `source_ip`: 有效的 IP 地址格式
- `content`: 不能为空
- `message_type`: 必须是有效的枚举值
- `created_at`: 有效的 ISO8601 时间戳

## 示例代码

### Rust 创建 UserMessage

```rust
use crate::chat_handler::chat_model::{UserMessage, MessageContent, MessageType};

// 创建纯文本消息
let message = UserMessage::from_text(
    "user123".to_string(),
    "192.168.1.100".to_string(),
    "你好，我想了解一下这个系统".to_string()
);

// 创建命令消息
let command_message = UserMessage::from_command(
    "user123".to_string(),
    "192.168.1.100".to_string(),
    "help".to_string(),
    vec!["list".to_string(), "commands".to_string()]
);

// 创建带元数据的消息
let message_with_metadata = UserMessage::new(
    "user123".to_string(),
    "192.168.1.100".to_string(),
    MessageContent::Text("请帮我分析这个数据".to_string())
)
.with_message_type(MessageType::Chat)
.with_metadata("priority".to_string(), serde_json::json!("high"));
```

### JavaScript 发送 UserMessage

```javascript
// 发送纯文本消息
const textMessage = {
    user: "user123",
    source_ip: "192.168.1.100",
    content: "你好，我想了解一下这个系统",
    message_type: "Chat",
    metadata: {},
    created_at: new Date().toISOString()
};

fetch('/api/chat', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
    },
    body: JSON.stringify(textMessage)
})
.then(response => response.json())
.then(data => console.log(data));

// 发送富文本消息
const richTextMessage = {
    user: "user123",
    source_ip: "192.168.1.100",
    content: {
        text: "# 标题\n这是一个**粗体**文本",
        format: "Markdown"
    },
    message_type: "Chat",
    metadata: {},
    created_at: new Date().toISOString()
};