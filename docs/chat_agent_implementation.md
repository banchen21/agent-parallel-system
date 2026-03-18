# Chat Agent 当前实现

本文档聚焦已经写进代码的实现细节。

## 当前对外能力

### 1. WebSocket 聊天

- 路径：`GET /ws/chat?token=<access_token>`
- 认证：查询参数中的 JWT Access Token

客户端发送格式：

```json
{
  "content": "你好",
  "device_type": "web"
}
```

服务端返回格式：

1. `thinking`
2. `message`
3. `task_progress`
4. `error`

### 2. 历史消息查询

- 路径：`GET /api/v1/message`
- 认证：Bearer Token

## 一次 WebSocket 会话内部会做什么

1. 订阅 Channel 通知
2. 启动 ping 保活
3. 收到用户文本后先回 `thinking`
4. 持久化用户消息
5. 调用 `ChatAgent`
6. 持久化 AI 回复
7. 推送 `message`

## 实时任务通知实现

1. WS 会话会订阅 Channel 广播
2. 只有 `任务系统` 发送者的广播会被转成 `task_progress`
3. 这样前端无需刷新就能看到任务相关通知

## 当前实现中的非目标

以下内容并不属于当前已实现功能：

1. HTTP 流式聊天发送
2. 插件化消息处理器注册表
3. 通用多模态消息工作流
