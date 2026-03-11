# 前端超时问题解决方案

## 问题分析

### 当前问题

1. **同步等待响应**：[`handle_message`](src/chat_handler/chat.rs:22) 函数使用 `chat_agent.send(...).await` 同步等待 ChatAgent 处理完成
2. **无流式响应**：虽然 `/stream` 端点存在，但实际使用的是相同的同步处理函数
3. **长时间阻塞**：ChatAgent 处理 LLM 响应可能需要数秒到数十秒，导致前端超时

### 代码问题点

```rust
// src/chat_handler/chat.rs 第112-116行
let chat_agent_response = chat_agent
    .send(OtherUserMessage {
        content: user_message.clone(),
    })
    .await;  // 这里会阻塞等待 ChatAgent 处理完成
```

## 解决方案

### 方案一：实现真正的 SSE 流式响应（推荐）

#### 1. 修改 ChatAgent 支持流式输出

```rust
// src/chat_handler/chat_agent.rs

use futures::stream::{self, Stream, StreamExt};
use actix_web::web::Bytes;

// 添加流式消息类型
#[derive(Debug, Clone)]
pub enum ChatAgentMessage {
    Regular(UserMessage),
    StreamRequest {
        user: String,
        content: String,
        session_id: Option<String>,
    },
}

// 修改 ChatAgent 处理逻辑
impl ChatAgent {
    pub async fn handle_stream_request(
        &mut self,
        request: StreamRequest,
    ) -> impl Stream<Item = Result<Bytes, actix_web::Error>> {
        // 创建 SSE 流
        let (tx, rx) = stream::channel(100);

        // 异步处理请求
        tokio::spawn(async move {
            let mut stream = self.openai_client.chat_completion_stream(request).await;

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => {
                        // 发送 SSE 数据块
                        let data = format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap());
                        if let Err(e) = tx.send(Ok(Bytes::from(data))).await {
                            error!("发送 SSE 数据块失败: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("获取 LLM 响应块失败: {}", e);
                        let error_data = format!("data: {{\"error\": \"{}\"}}\n\n", e);
                        if let Err(e) = tx.send(Ok(Bytes::from(error_data))).await {
                            error!("发送错误数据块失败: {}", e);
                        }
                        break;
                    }
                }
            }

            // 发送结束标记
            let end_data = "data: [DONE]\n\n";
            let _ = tx.send(Ok(Bytes::from(end_data))).await;
        });

        rx
    }
}
```

#### 2. 修改 chat.rs 实现流式处理

```rust
// src/chat_handler/chat.rs

use actix_web::web::Sse;
use futures::StreamExt;

pub async fn handle_stream(
    chat_request: web::Json<ChatRequest>,
    chat_agent: web::Data<Addr<ChatAgent>>,
    channel_manager: web::Data<Addr<ChannelManagerActor>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let client_ip = req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 保存消息到数据库
    let user_message = UserMessage {
        user: chat_request.user.clone(),
        content: MessageContent::Text(chat_request.content.clone()),
        message_type: MessageType::Chat,
        source_ip: client_ip,
        metadata: chat_request.metadata.clone(),
        created_at: Local::now(),
    };

    let save_message = SaveMessage { message: user_message };
    let _ = channel_manager.send(save_message).await;

    // 转换为流式请求
    let stream_request = StreamRequest {
        user: chat_request.user.clone(),
        content: chat_request.content.clone(),
        session_id: chat_request.metadata.get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    };

    // 返回 SSE 流
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .streaming(Sse::new(chat_agent.send(stream_request).await.unwrap())))
}
```

#### 3. 更新路由配置

```rust
// src/main.rs

fn configure_api_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/v1")
        .route(
            "/message",
            web::post().to(chat_handler::chat::handle_message),
        )
        .route(
            "/stream",
            web::post().to(chat_handler::chat::handle_stream),
        )
    );
}
```

### 方案二：添加心跳机制（临时方案）

如果暂时无法实现流式响应，可以添加心跳机制：

```rust
// 在 handle_message 中添加心跳
pub async fn handle_message(...) -> ActixResult<HttpResponse> {
    // ... 现有代码 ...

    // 发送处理中状态
    let _ = channel_manager.send(SaveMessage {
        message: UserMessage {
            user: chat_request.user.clone(),
            content: MessageContent::Text("[系统] 正在处理您的请求，请稍候...".to_string()),
            message_type: MessageType::System,
            source_ip: client_ip,
            metadata: chat_request.metadata.clone(),
            created_at: Local::now(),
        },
    }).await;

    // 等待 ChatAgent 处理
    let chat_agent_response = chat_agent.send(...).await;

    // ... 返回响应 ...
}
```

## 前端实现建议

### SSE 客户端示例

```javascript
async function sendMessage(content) {
    const response = await fetch('/api/v1/stream', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({
            user: 'user123',
            content: content,
            stream: true
        })
    });

    const reader = response.body.getReader();
    const decoder = new TextDecoder();

    while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        const chunk = decoder.decode(value);
        const lines = chunk.split('\n');

        for (const line of lines) {
            if (line.startsWith('data: ')) {
                const data = line.slice(6);
                if (data === '[DONE]') break;

                try {
                    const parsed = JSON.parse(data);
                    console.log('收到数据块:', parsed);
                    // 更新UI显示
                    updateChatDisplay(parsed.content);
                } catch (e) {
                    console.error('解析数据失败:', e);
                }
            }
        }
    }
}
```

## 优先级建议

1. **高优先级**：实现真正的 SSE 流式响应（方案一）
2. **中优先级**：添加处理中状态提示（方案二）
3. **低优先级**：优化超时设置和重试机制

## 相关文件

- [`src/chat_handler/chat.rs`](src/chat_handler/chat.rs:1) - 当前处理函数
- [`src/chat_handler/chat_agent.rs`](src/chat_handler/chat_agent.rs:1) - ChatAgent 实现
- [`src/main.rs`](src/main.rs:173) - 路由配置
- [`docs/streaming_api_guide.md`](docs/streaming_api_guide.md:1) - 流式API文档