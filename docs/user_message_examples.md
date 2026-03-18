# 消息示例

本文档给出当前项目里最常见的消息输入输出示例。

## 1. WebSocket 发送普通聊天消息

```json
{
  "content": "你好",
  "device_type": "web"
}
```

## 2. WebSocket 发送任务型请求

```json
{
  "content": "请帮我检查 MCP 工具为什么没有执行，并修复它",
  "device_type": "web"
}
```

## 3. 服务端返回 thinking

```json
{
  "type": "thinking"
}
```

## 4. 服务端返回聊天回复

```json
{
  "type": "message",
  "sender": "AI",
  "content": "我先检查 MCP 工具的定义和执行链路。",
  "created_at": "2026-03-18T08:00:00+00:00"
}
```

## 5. 服务端返回任务通知

```json
{
  "type": "task_progress",
  "sender": "任务系统",
  "content": "用户已接收审阅结果，任务完成。",
  "created_at": "2026-03-18T08:01:00+00:00"
}
```

## 6. 服务端返回错误

```json
{
  "type": "error",
  "message": "无效的 token"
}
```
