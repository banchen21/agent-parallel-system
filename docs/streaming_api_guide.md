# 实时接口指南

当前项目的实时接口分为两类：

1. WebSocket：聊天与任务通知
2. SSE：日志流

## 一、WebSocket 聊天接口

### 连接地址

```text
GET /ws/chat?token=<access_token>
```

### 客户端发送格式

```json
{
  "content": "请帮我检查任务失败链路",
  "device_type": "web"
}
```

### 服务端返回格式

1. `thinking`
2. `message`
3. `task_progress`
4. `error`

## 二、SSE 日志接口

### 连接地址

```text
GET /logs/stream?token=<access_token>
```

### 响应格式

服务端按 SSE 标准逐条推送：

```text
data: {"level":"INFO","message":"..."}

```

## 三、选择建议

### 使用 WebSocket 的场景

1. 聊天发送与回复
2. 任务执行中的实时通知
3. 需要双向通信的页面

### 使用 SSE 的场景

1. 只读日志面板
2. 后端 tracing 输出观察
3. 排障和运行监控

## 四、当前实现约束

1. 聊天主链路已经切到 WebSocket
2. SSE 不提供聊天结果
3. 任务页实时刷新依赖 WebSocket `task_progress`
