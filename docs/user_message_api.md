# 消息接口与数据结构

当前项目中的消息主要分成两类：

1. HTTP 历史消息查询结果
2. WebSocket 实时消息帧

## 一、历史消息接口

### 路径

```text
GET /api/v1/message
```

### 认证

Bearer Access Token。

### 查询参数

1. `limit`
2. `before`

### 返回格式

```json
[
  {
    "sender": "banchen",
    "content": "你好",
    "created_at": "2026-03-18T08:00:00Z"
  }
]
```

## 二、WebSocket 输入结构

客户端发送到 `/ws/chat` 的结构是：

```json
{
  "content": "请帮我分析任务失败链路",
  "device_type": "web"
}
```

## 三、WebSocket 输出结构

### 1. thinking

```json
{
  "type": "thinking"
}
```

### 2. message

```json
{
  "type": "message",
  "sender": "AI",
  "content": "这是回复内容",
  "created_at": "2026-03-18T08:00:00+00:00"
}
```

### 3. task_progress

```json
{
  "type": "task_progress",
  "sender": "任务系统",
  "content": "任务进入审阅决策阶段",
  "created_at": "2026-03-18T08:00:00+00:00"
}
```

### 4. error

```json
{
  "type": "error",
  "message": "消息格式错误，请发送 JSON"
}
```

## 四、注意点

1. 当前聊天发送主路径是 WebSocket，不是 HTTP POST
2. 任务通知也通过 WebSocket 返回
3. 历史消息接口返回的是简化结构
