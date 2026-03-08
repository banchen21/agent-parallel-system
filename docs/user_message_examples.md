# 用户消息格式示例

本文档展示了如何使用新的 `UserMessage` 结构体来发送各种格式的消息。

## 基本结构

```json
{
  "user": "用户ID",
  "content": "消息内容",
  "message_type": "Chat",
  "priority": "Normal",
  "recipient": null,
  "metadata": {},
  "expires_at": null
}
```

## 消息内容格式

### 1. 纯文本消息

```json
{
  "user": "user123",
  "content": "Hello, world!"
}
```

### 2. 富文本消息

```json
{
  "user": "user123",
  "content": {
    "text": "这是一个 **粗体** 文本",
    "format": "Markdown"
  }
}
```

支持的格式：
- `Plain` - 纯文本
- `Markdown` - Markdown 格式
- `Html` - HTML 格式
- `Json` - JSON 格式

### 3. 结构化消息

```json
{
  "user": "user123",
  "content": {
    "message_type": "form_submission",
    "data": {
      "name": "张三",
      "email": "zhangsan@example.com",
      "message": "我想咨询产品信息"
    }
  }
}
```

### 4. 多媒体消息

```json
{
  "user": "user123",
  "content": {
    "media_type": "Image",
    "url": "https://example.com/image.jpg",
    "caption": "这是一张图片",
    "metadata": {
      "width": 800,
      "height": 600,
      "size": "150KB"
    }
  }
}
```

支持的媒体类型：
- `Image` - 图片
- `Video` - 视频
- `Audio` - 音频
- `Document` - 文档
- `Link` - 链接

### 5. 命令消息

```json
{
  "user": "user123",
  "content": {
    "command": "create_user",
    "args": ["--name", "张三", "--email", "zhangsan@example.com"]
  }
}
```

### 6. 文件消息

```json
{
  "user": "user123",
  "content": {
    "filename": "document.pdf",
    "content_type": "application/pdf",
    "size": 1024000,
    "url": "https://example.com/files/document.pdf"
  }
}
```

## 消息类型

- `Chat` - 聊天消息（默认）
- `Task` - 任务指令
- `System` - 系统命令
- `Query` - 查询请求
- `Response` - 响应消息

## 优先级

- `Low` - 低优先级
- `Normal` - 普通优先级（默认）
- `High` - 高优先级
- `Critical` - 关键优先级

## 完整示例

### 高优先级任务消息

```json
{
  "user": "admin",
  "content": {
    "command": "system_backup",
    "args": ["--full", "--compress"]
  },
  "message_type": "Task",
  "priority": "High",
  "recipient": "system_manager",
  "metadata": {
    "request_id": "req_12345",
    "timeout": 3600
  },
  "expires_at": "2024-01-01T12:00:00Z"
}
```

### 带图片的聊天消息

```json
{
  "user": "user123",
  "content": {
    "media_type": "Image",
    "url": "https://example.com/photo.jpg",
    "caption": "看看我拍的照片！"
  },
  "message_type": "Chat",
  "priority": "Normal"
}
```

### 表单提交消息

```json
{
  "user": "user456",
  "content": {
    "message_type": "contact_form",
    "data": {
      "name": "李四",
      "phone": "13800138000",
      "subject": "产品咨询",
      "message": "我想了解你们的产品价格和功能"
    }
  },
  "message_type": "Chat",
  "metadata": {
    "form_id": "contact_001",
    "source": "website"
  }
}
```

## API 端点

- `POST /message` - 通用消息接口
- `POST /chat` - 聊天消息接口
- `POST /task` - 任务消息接口
- `POST /system` - 系统消息接口

## 向后兼容

系统仍然支持简单的消息格式：

```json
{
  "user": "user123",
  "content": "简单文本消息"
}
```

这种格式会自动转换为新的 `UserMessage` 结构体。

## 错误处理

如果消息格式不正确，系统会返回错误响应：

```json
{
  "success": false,
  "error": "Invalid message format: ..."
}
```

## 注意事项

1. `user` 字段是必需的
2. `content` 字段是必需的，可以是字符串或对象
3. 其他字段都是可选的
4. 时间字段使用 ISO 8601 格式
5. 元数据字段可以包含任意的 JSON 数据