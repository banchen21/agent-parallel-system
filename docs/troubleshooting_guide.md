# ChatAgent 故障排除指南

## 概述

本指南帮助诊断和解决ChatAgent API中的常见问题，特别是"Requested application data is not configured correctly"错误。

## 常见问题及解决方案

### 1. "Requested application data is not configured correctly"

#### 问题症状
```bash
./test_chinese_messages.sh
📤 响应内容:
Requested application data is not configured correctly. View/enable debug logs for more details.
```

#### 可能原因
1. ChatAgent未正确注入到HTTP服务器
2. 参数顺序不匹配
3. 类型注解缺失

#### 解决方案

**步骤1: 检查main.rs配置**
确保ChatAgent已正确添加到HTTP服务器配置：

```rust
// src/main.rs
HttpServer::new(move || {
    App::new()
        .app_data(web::Data::new(pool.clone()))
        .app_data(channel_manager.clone())
        .app_data(web::Data::new(chat_agent.clone()))  // 必须用Data::new()包装
        .configure(|cfg| configure_api_routes(cfg))
})
```

**重要提示**: ChatAgent必须用`web::Data::new()`包装，否则会出现依赖注入失败。

**步骤2: 检查handle_message函数签名**
确保参数顺序正确：

```rust
// src/chat_handler/chat.rs
pub async fn handle_message(
    req_message: web::Json<ApiUserMessage>,        // 第一个参数
    chat_agent: web::Data<Addr<chat_agent::ChatAgent>>, // 第二个参数
    channel_manager: web::Data<Addr<ChannelManagerActor>>, // 第三个参数
) -> ActixResult<HttpResponse> {
```

**步骤3: 重启服务器**
```bash
# 停止当前服务器 (Ctrl+C)
# 重新启动
cargo run
```

**步骤4: 测试修复**
```bash
./test_fix.sh
```

### 2. OpenAI API相关错误

#### 问题症状
```json
"AI处理失败"
```
或
```json
"OpenAI API 调用失败: ..."
```

#### 解决方案

**检查环境变量:**
```bash
echo $OPENAI_API_KEY
echo $OPENAI_BASE_URL
```

**设置环境变量:**
```bash
export OPENAI_API_KEY="your-api-key-here"
export OPENAI_BASE_URL="https://api.openai.com/v1"
```

**测试API连接:**
```bash
curl -H "Authorization: Bearer $OPENAI_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Hello"}]}' \
     $OPENAI_BASE_URL/chat/completions
```

### 3. 数据库连接错误

#### 问题症状
```bash
数据库连接失败
```

#### 解决方案

**检查数据库URL:**
```bash
echo $DATABASE_URL
```

**设置数据库URL:**
```bash
export DATABASE_URL="postgresql://username:password@localhost/database_name"
```

**启动数据库服务:**
```bash
# PostgreSQL
sudo systemctl start postgresql

# 或使用Docker
docker run -d --name postgres -e POSTGRES_PASSWORD=password -p 5432:5432 postgres
```

### 4. 编译错误

#### 问题症状
```bash
error[E0277]: the trait bound `ChatAgent: Clone` is not satisfied
```

#### 解决方案

确保ChatAgent实现了Clone trait：

```rust
// src/chat_handler/chat_agent.rs
impl Clone for ChatAgent {
    fn clone(&self) -> Self {
        Self {
            openai_client: self.openai_client.clone(),
            channel_manager: self.channel_manager.clone(),
            config: self.config.clone(),
        }
    }
}
```

## 调试技巧

### 1. 启用详细日志

**Debug模式:**
```bash
RUST_LOG=debug cargo run
```

**特定模块日志:**
```bash
RUST_LOG=agent_parallel_system::chat_handler=debug cargo run
```

**Trace模式 (最详细):**
```bash
RUST_LOG=trace cargo run
```

### 2. 检查服务器状态

**基本健康检查:**
```bash
curl http://localhost:8000
```

**检查路由:**
```bash
curl -v http://localhost:8000/message
```

### 3. 手动测试API

**简单测试:**
```bash
curl -X POST -H "Content-Type: application/json" \
     -d '{"user":"test","content":"hello"}' \
     http://localhost:8000/message
```

**带详细信息的测试:**
```bash
curl -v -X POST -H "Content-Type: application/json" \
     -d '{"user":"test","content":"hello"}' \
     http://localhost:8000/message
```

### 4. 检查依赖

**查看Cargo.toml:**
```bash
cat Cargo.toml | grep -E "(actix|async-openai|sqlx)"
```

**检查依赖版本:**
```bash
cargo tree | grep -E "(actix|async-openai|sqlx)"
```

## 常见错误代码

| 错误信息 | 原因 | 解决方案 |
|---------|------|----------|
| `Requested application data is not configured correctly` | 依赖注入失败 | 检查参数顺序和app_data配置 |
| `Actor mailbox full` | Actor过载 | 增加邮箱大小或优化处理逻辑 |
| `Connection refused` | 服务未启动 | 启动相关服务 |
| `Authentication failed` | API密钥错误 | 检查环境变量配置 |
| `Database timeout` | 数据库连接超时 | 检查数据库状态和网络 |

## 性能问题诊断

### 1. 响应时间过长

**检查点:**
- OpenAI API响应时间
- 数据库查询时间
- 网络延迟

**优化建议:**
- 使用连接池
- 实现缓存机制
- 优化数据库查询

### 2. 内存使用过高

**检查点:**
- Actor消息积压
- 流式响应缓冲
- 内存泄漏

**优化建议:**
- 限制消息队列大小
- 及时释放资源
- 监控内存使用

## 预防性维护

### 1. 定期检查

**每日检查脚本:**
```bash
#!/bin/bash
# daily_check.sh
./test_fix.sh > daily_check_$(date +%Y%m%d).log 2>&1
if grep -q "error\|Error\|ERROR" daily_check_$(date +%Y%m%d).log; then
    echo "Daily check failed" | mail -s "System Alert" admin@example.com
fi
```

### 2. 监控设置

**系统监控:**
```bash
# 监控服务器状态
while true; do
    curl -s http://localhost:8000 > /dev/null
    if [ $? -ne 0 ]; then
        echo "Server down at $(date)" >> server_monitor.log
    fi
    sleep 60
done
```

### 3. 日志管理

**日志轮转:**
```bash
# logrotate.conf
/path/to/agent-parallel-system/logs/*.log {
    daily
    rotate 7
    compress
    missingok
    notifempty
}
```

## 联系支持

如果问题仍然存在：

1. **收集信息:**
   - 错误日志
   - 系统环境信息
   - 重现步骤

2. **创建问题报告:**
   ```bash
   cargo run --version > system_info.txt
   rustc --version >> system_info.txt
   uname -a >> system_info.txt
   ```

3. **寻求帮助:**
   - 查看项目文档
   - 搜索已知问题
   - 提交Issue

## 总结

通过系统性的故障排除方法，大多数ChatAgent问题都可以快速定位和解决。关键是要：

1. **仔细阅读错误信息**
2. **逐步排查可能原因**
3. **使用调试工具获取更多信息**
4. **记录解决方案供将来参考**

定期维护和监控可以预防许多常见问题的发生。