# ChatAgent API 测试指南

## 概述

本文档提供了测试ChatAgent消息API的完整指南，包括测试脚本的使用方法和故障排除。

## 测试脚本

### 1. 快速测试脚本 (`quick_test.sh`)

用于快速验证API基本功能的简化脚本。

**使用方法:**
```bash
./quick_test.sh
```

**功能:**
- 检查服务器状态
- 发送简单测试消息
- 验证响应内容
- 测量响应时间

### 2. 完整测试脚本 (`test_message_api.sh`)

用于全面测试API功能的详细脚本。

**使用方法:**
```bash
./test_message_api.sh
```

**测试内容:**
1. 简单文本消息
2. 复杂结构化消息
3. 命令消息
4. 错误处理（无效JSON）
5. 性能测试（响应时间）

## 测试消息格式

### 简单文本消息
```json
{
    "user": "test_user",
    "content": "你好，请简单介绍一下你自己"
}
```

### 复杂结构化消息
```json
{
    "user": "test_user",
    "content": {
        "text": "Rust编程语言的主要特点是什么？请详细说明。",
        "format": "Plain"
    },
    "message_type": "Chat",
    "priority": "Normal",
    "metadata": {
        "source": "api_test",
        "version": "1.0"
    }
}
```

### 命令消息
```json
{
    "user": "test_user",
    "content": {
        "command": "帮助",
        "args": []
    },
    "message_type": "Task",
    "priority": "High"
}
```

## 运行测试

### 前置条件

1. **启动服务器:**
   ```bash
   cargo run
   ```

2. **设置环境变量 (可选):**
   ```bash
   export OPENAI_API_KEY="your-api-key"
   export OPENAI_BASE_URL="https://api.openai.com/v1"
   export DATABASE_URL="postgresql://user:password@localhost/dbname"
   ```

3. **启用Debug日志 (推荐):**
   ```bash
   RUST_LOG=debug cargo run
   ```

### 执行测试

1. **快速测试:**
   ```bash
   ./quick_test.sh
   ```

2. **完整测试:**
   ```bash
   ./test_message_api.sh
   ```

## 预期响应

### 成功响应示例
```json
{
    "response_id": "123e4567-e89b-12d3-a456-426614174000",
    "content": "Hello! I'm an AI assistant powered by ChatAgent...",
    "response_type": "Text",
    "session_id": "456e7890-e89b-12d3-a456-426614174001",
    "processing_time": "2.345s",
    "metadata": {
        "message_classification": {
            "message_type": "Chat",
            "confidence": 0.8,
            "reasoning": "用户消息处理",
            "extracted_entities": [],
            "suggested_actions": []
        },
        "suggested_actions": [],
        "confidence_score": 0.8
    }
}
```

### 错误响应示例
```json
"AI处理失败"
```
或
```json
"服务器内部错误"
```

## Debug输出

当启用debug日志时，您将看到详细的处理过程：

```
🏗️ 开始创建ChatAgent实例
🎬 ChatAgent Actor 启动中...
📨 ChatAgent接收到用户消息: ID=xxx, 用户=test_user, 内容长度=25 字符
🚀 开始处理流式响应，提示词: Hello, say hi back
📤 发送OpenAI请求，模型: gpt-3.5-turbo，最大令牌数: 512
📡 开始接收流式响应...
📦 接收到第 1 个数据块
📝 流式内容片段: "Hello"
📊 累积内容长度: 5 字符
🎉 流式响应完成，总数据块数: 3，总内容长度: 12 字符
🎯 ChatAgent响应创建完成: ID=xxx, 处理时间=1.234s
```

## 故障排除

### 常见问题

1. **服务器未运行**
   ```
   ❌ 服务器未运行，请先启动: cargo run
   ```
   **解决方案:** 启动服务器 `cargo run`

2. **OpenAI API错误**
   ```
   ❌ 测试失败: 收到错误响应
   ```
   **解决方案:** 检查环境变量 `OPENAI_API_KEY` 和 `OPENAI_BASE_URL`

3. **数据库连接错误**
   ```
   数据库连接失败
   ```
   **解决方案:** 检查 `DATABASE_URL` 环境变量和数据库服务

4. **响应时间过长**
   ```
   ⚠️ 测试5警告: 响应时间较长 (15000ms)
   ```
   **解决方案:** 检查网络连接和OpenAI API状态

### 调试技巧

1. **启用详细日志:**
   ```bash
   RUST_LOG=debug cargo run
   ```

2. **检查特定模块日志:**
   ```bash
   RUST_LOG=agent_parallel_system::chat_handler=debug cargo run
   ```

3. **使用curl手动测试:**
   ```bash
   curl -X POST -H "Content-Type: application/json" \
        -d '{"user":"test","content":"Hello"}' \
        http://localhost:8000/message
   ```

4. **检查服务器状态:**
   ```bash
   curl http://localhost:8000
   ```

## 性能基准

### 预期性能指标

- **响应时间:** < 10秒 (包含OpenAI API调用)
- **并发处理:** 支持多个同时请求
- **错误率:** < 1%

### 性能优化建议

1. **使用连接池**优化数据库连接
2. **缓存常见响应**减少API调用
3. **异步处理**提高并发性能
4. **监控资源使用**及时发现问题

## 自动化测试

### CI/CD集成

可以将测试脚本集成到CI/CD流水线中：

```yaml
# .github/workflows/test.yml
name: API Test
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Start Server
        run: |
          cargo run &
          sleep 10
      - name: Run Tests
        run: ./test_message_api.sh
```

### 监控和告警

设置监控脚本定期检查API状态：

```bash
#!/bin/bash
# monitor_api.sh
response=$(curl -s -X POST -H "Content-Type: application/json" \
    -d '{"user":"monitor","content":"health check"}' \
    http://localhost:8000/message)

if echo "$response" | grep -q "error\|Error\|ERROR"; then
    echo "API健康检查失败" | mail -s "API Alert" admin@example.com
fi
```

## 总结

通过使用这些测试脚本，您可以：

1. **验证API功能**确保消息处理正常
2. **监控性能**及时发现性能问题
3. **测试错误处理**确保系统健壮性
4. **调试问题**通过详细日志定位问题

定期运行这些测试有助于保持ChatAgent API的稳定性和可靠性。