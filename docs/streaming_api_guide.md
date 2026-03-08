# 流式API使用指南

## 概述

本系统支持Server-Sent Events (SSE) 流式响应，允许实时接收AI生成的响应内容，提供更好的用户体验。

## API端点

### 流式消息处理

**端点**: `POST /stream`

**描述**: 处理用户消息并返回流式响应或普通响应

## 请求格式

### 请求体结构

```json
{
  "user": "string",        // 用户ID
  "content": "string",     // 消息内容
  "stream": boolean,       // 是否启用流式响应
  "session_id": "string"   // 可选：会话ID
}
```

### 请求示例

#### 流式响应请求
```bash
curl -X POST "http://localhost:8080/stream" \
  -H "Content-Type: application/json" \
  -d '{
    "user": "test_user",
    "content": "你好，请介绍一下Rust编程语言",
    "stream": true,
    "session_id": "session_001"
  }'
```

#### 非流式响应请求
```bash
curl -X POST "http://localhost:8080/stream" \
  -H "Content-Type: application/json" \
  -d '{
    "user": "test_user",
    "content": "什么是异步编程？",
    "stream": false
  }'
```

## 响应格式

### 流式响应 (stream=true)

当 `stream=true` 时，响应使用Server-Sent Events格式：

```
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
```

#### 数据块格式

每个数据块包含以下字段：

```json
{
  "id": "string",                    // 块唯一标识符
  "content": "string",               // 当前累积的内容
  "finished": boolean,               // 是否为最后一个块
  "timestamp": "2024-01-01T00:00:00Z", // 时间戳
  "metadata": {                      // 可选元数据
    "session_id": "string",
    "user_id": "string",
    "chunk_index": number
  }
}
```

#### SSE格式示例

```
data: {"id":"chunk-001","content":"你","finished":false,"timestamp":"2024-01-01T00:00:00Z","metadata":{"session_id":"session_001","user_id":"test_user","chunk_index":0}}

data: {"id":"chunk-001","content":"你好","finished":false,"timestamp":"2024-01-01T00:00:01Z","metadata":{"session_id":"session_001","user_id":"test_user","chunk_index":1}}

data: {"id":"chunk-001","content":"你好！Rust","finished":false,"timestamp":"2024-01-01T00:00:02Z","metadata":{"session_id":"session_001","user_id":"test_user","chunk_index":2}}

data: {"id":"chunk-001","content":"你好！Rust是一门系统编程语言...","finished":true,"timestamp":"2024-01-01T00:00:03Z","metadata":{"session_id":"session_001","user_id":"test_user","chunk_index":3}}
```

### 非流式响应 (stream=false)

当 `stream=false` 时，返回标准的JSON响应：

```json
{
  "content": "完整的响应内容",
  "metadata": {
    "response_type": "chat",
    "model": "gpt-3.5-turbo",
    "session_id": "session_001"
  }
}
```

## 客户端实现

### JavaScript示例

```javascript
// 流式响应处理
async function streamResponse(message) {
  const response = await fetch('/stream', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      user: 'user123',
      content: message,
      stream: true,
      session_id: 'session_' + Date.now()
    })
  });

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let fullContent = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    const chunk = decoder.decode(value);
    const lines = chunk.split('\n');
    
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = JSON.parse(line.slice(6));
        fullContent = data.content;
        
        // 更新UI显示当前内容
        updateUI(data.content);
        
        if (data.finished) {
          console.log('流式响应完成');
        }
      }
    }
  }
  
  return fullContent;
}

// 非流式响应处理
async function normalResponse(message) {
  const response = await fetch('/stream', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      user: 'user123',
      content: message,
      stream: false
    })
  });

  const data = await response.json();
  return data.content;
}
```

### Python示例

```python
import requests
import json

def stream_response(message):
    """流式响应处理"""
    response = requests.post(
        'http://localhost:8080/stream',
        json={
            'user': 'test_user',
            'content': message,
            'stream': True,
            'session_id': 'python_session'
        },
        stream=True
    )
    
    full_content = ""
    for line in response.iter_lines():
        if line:
            line = line.decode('utf-8')
            if line.startswith('data: '):
                data = json.loads(line[6:])
                full_content = data['content']
                print(f"收到块: {data['content']}")
                
                if data['finished']:
                    print("流式响应完成")
                    break
    
    return full_content

def normal_response(message):
    """非流式响应处理"""
    response = requests.post(
        'http://localhost:8080/stream',
        json={
            'user': 'test_user',
            'content': message,
            'stream': False
        }
    )
    
    return response.json()['content']
```

## 测试

### 使用提供的测试脚本

```bash
# 运行流式API测试
./test_streaming_api.sh
```

### 手动测试

1. **启动服务器**
   ```bash
   cargo run
   ```

2. **测试流式响应**
   ```bash
   curl -X POST "http://localhost:8000/stream" \
     -H "Content-Type: application/json" \
     -d '{
       "user": "test_user",
       "content": "你好，请介绍一下Rust",
       "stream": true
     }' --no-buffer
   ```

3. **测试非流式响应**
   ```bash
   curl -X POST "http://localhost:8080/stream" \
     -H "Content-Type: application/json" \
     -d '{
       "user": "test_user", 
       "content": "什么是异步编程？",
       "stream": false
     }'
   ```

## 特性

- **实时流式响应**: 支持逐字符或逐词的实时响应
- **会话管理**: 支持会话ID跟踪对话上下文
- **错误处理**: 完善的错误处理和错误流式传输
- **中文支持**: 完全支持中文内容的流式传输
- **兼容性**: 同时支持流式和非流式响应模式

## 注意事项

1. **连接管理**: 流式响应使用长连接，客户端需要正确处理连接断开的情况
2. **内存使用**: 流式响应可以减少客户端内存使用，特别适合长文本响应
3. **超时处理**: 建议设置合理的超时时间，避免长时间等待
4. **错误恢复**: 实现重连机制以处理网络中断

## 故障排除

### 常见问题

1. **流式响应中断**
   - 检查网络连接
   - 验证服务器是否正常运行
   - 查看服务器日志

2. **中文乱码**
   - 确保客户端正确处理UTF-8编码
   - 检查Content-Type头部设置

3. **性能问题**
   - 监控服务器资源使用
   - 调整流式块的大小和发送频率

### 调试技巧

1. **启用调试日志**
   ```bash
   RUST_LOG=debug cargo run
   ```

2. **监控网络流量**
   ```bash
   # 使用tcpdump监控HTTP流量
   sudo tcpdump -i lo port 8080
   ```

3. **检查响应头**
   ```bash
   curl -I -X POST "http://localhost:8080/stream" \
     -H "Content-Type: application/json" \
     -d '{"user":"test","content":"test","stream":true}'