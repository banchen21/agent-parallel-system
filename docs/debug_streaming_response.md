# 实时流调试指南

当前项目中的实时能力主要有两类：

1. WebSocket 聊天流
2. SSE 日志流

## 一、调试 WebSocket 聊天流

入口：`GET /ws/chat?token=<access_token>`

### 重点检查项

1. token 是否有效
2. 前端是否发送了合法 JSON
3. `ChatWsSession.started()` 是否成功订阅 Channel
4. `ChatAgent` 是否正常返回响应

### 典型输出类型

1. `thinking`
2. `message`
3. `task_progress`
4. `error`

### 常见故障

#### 1. 只能连接，收不到消息

排查：

1. 检查请求是否真正发送文本帧
2. 检查后端日志中是否有 `WS 聊天会话已建立`
3. 检查 `ChatAgent` 调用是否报错

#### 2. 聊天能回，任务通知不回

排查：

1. 确认 `SubscribeChannelNotify` 已发送
2. 确认 `ChannelEvent.user == "任务系统"`
3. 确认前端识别 `task_progress`

## 二、调试 SSE 日志流

入口：`GET /logs/stream?token=<access_token>`

### 检查顺序

1. token 是否有效
2. `log_broadcaster` 是否初始化
3. 浏览器端是否使用 `EventSource`
4. 中间代理是否缓冲响应

## 三、当前实现注意点

1. WebSocket 是主聊天通道
2. SSE 只负责日志流，不负责聊天回复
3. 任务通知通过 WebSocket 返回
