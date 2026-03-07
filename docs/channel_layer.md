# 通道层 (Channel Layer) 文档

## 概述

通道层是 Agent Parallel System 的核心组件，负责接收、处理和分发来自不同来源的消息。它提供了一个统一的消息处理框架，支持优先级队列、多种消息类型和可扩展的处理器架构。

## 架构设计

### 核心组件

1. **消息类型系统** (`types.rs`)
   - `Message`: 核心消息结构
   - `MessageSource`: 消息来源枚举 (Api, Terminal, Internal)
   - `MessageType`: 消息类型枚举 (Chat, Task, System, Query, Response)
   - `MessagePriority`: 消息优先级 (Low, Normal, High, Critical)
   - `MessageResult`: 消息处理结果

2. **消息处理器** (`handler.rs`)
   - `MessageHandler`: 处理器接口
   - `HandlerRegistry`: 处理器注册表
   - `ChatHandler`: 聊天消息处理器
   - `TaskHandler`: 任务消息处理器
   - `SystemHandler`: 系统消息处理器

3. **通道管理器** (`manager.rs`)
   - `ChannelManager`: 通道层核心管理器
   - 优先级队列管理
   - 工作线程池
   - 消息统计和监控

4. **消息接收器** (`receiver.rs`)
   - `ApiReceiver`: HTTP API 消息接收器
   - `TerminalReceiver`: 终端消息接收器
   - `ReceiverFactory`: 接收器工厂

5. **消息持久化** (`persistence.rs`)
   - `MessagePersistence`: 数据库持久化管理器
   - PostgreSQL 消息表管理
   - 消息历史记录查询
   - 统计信息收集

## 功能特性

### 1. 多源消息接收
- **HTTP API**: 通过 RESTful API 接收消息
- **终端接口**: 支持命令行输入
- **内部消息**: 系统内部组件间通信

### 2. 优先级队列
- 四个优先级级别：Critical > High > Normal > Low
- 每个优先级独立队列
- 高优先级消息优先处理

### 3. 可扩展处理器
- 插件式处理器架构
- 支持自定义消息处理器
- 处理器优先级和并发控制

### 4. 消息持久化
- PostgreSQL 数据库存储
- 完整的消息生命周期跟踪
- 消息历史记录和统计

### 5. 监控和统计
- 实时消息处理统计
- 性能指标收集
- 健康检查接口

## API 接口

### 通用消息接口
```
POST /message
Content-Type: application/json

{
  "content": "消息内容",
  "message_type": "chat|task|system|query",
  "priority": "low|normal|high|critical",
  "sender": "发送者标识",
  "recipient": "接收者标识（可选）",
  "metadata": {}
}
```

### 专用接口

#### 聊天消息
```
POST /chat
{
  "content": "你好，我想咨询一个问题",
  "sender": "user123"
}
```

#### 任务消息
```
POST /task
{
  "content": "请帮我分析这个数据",
  "sender": "user456",
  "priority": "high"
}
```

#### 系统消息
```
POST /system
{
  "content": "status",
  "sender": "admin"
}
```

### 监控接口

#### 健康检查
```
GET /health
```

#### 统计信息
```
GET /stats
```

## 数据库表结构

### messages 表

| 字段名 | 类型 | 描述 |
|--------|------|------|
| id | UUID | 消息唯一标识 |
| source | VARCHAR(20) | 消息来源 |
| message_type | VARCHAR(20) | 消息类型 |
| priority | INTEGER | 优先级 (1-4) |
| sender | VARCHAR(255) | 发送者 |
| recipient | VARCHAR(255) | 接收者 |
| content | TEXT | 消息内容 |
| metadata | JSONB | 元数据 |
| created_at | TIMESTAMPTZ | 创建时间 |
| processed_at | TIMESTAMPTZ | 处理时间 |
| expires_at | TIMESTAMPTZ | 过期时间 |
| status | VARCHAR(20) | 处理状态 |
| error_message | TEXT | 错误信息 |
| retry_count | INTEGER | 重试次数 |
| result_content | TEXT | 处理结果 |

## 使用示例

### 1. 发送聊天消息

```bash
curl -X POST http://localhost:8001/chat \
  -H "Content-Type: application/json" \
  -d '{
    "content": "你好，我想了解一下这个系统",
    "sender": "user123"
  }'
```

### 2. 发送任务消息

```bash
curl -X POST http://localhost:8001/task \
  -H "Content-Type: application/json" \
  -d '{
    "content": "请帮我分析用户行为数据",
    "sender": "user456",
    "priority": "high"
  }'
```

### 3. 系统状态查询

```bash
curl -X POST http://localhost:8001/system \
  -H "Content-Type: application/json" \
  -d '{
    "content": "status",
    "sender": "admin"
  }'
```

### 4. 获取统计信息

```bash
curl http://localhost:8001/stats
```

## 配置选项

### ChannelConfig

```rust
ChannelConfig {
    max_queue_size: 10000,        // 最大队列大小
    worker_threads: 4,            // 工作线程数
    message_timeout_seconds: 300, // 消息超时时间
    enable_persistence: true,     // 启用持久化
    max_retries: 3,               // 最大重试次数
}
```

## 扩展开发

### 自定义消息处理器

```rust
use async_trait::async_trait;
use crate::channel::{MessageHandler, Message, MessageResult};

pub struct CustomHandler {
    // 处理器状态
}

#[async_trait]
impl MessageHandler for CustomHandler {
    fn name(&self) -> &str {
        "CustomHandler"
    }
    
    async fn can_handle(&self, message: &Message) -> bool {
        // 判断是否能处理该消息
        message.message_type == MessageType::Task
    }
    
    async fn handle(&self, message: Message) -> Result<MessageResult> {
        // 处理消息逻辑
        Ok(MessageResult::success(
            message.id,
            "自定义处理完成".to_string()
        ))
    }
    
    fn priority(&self) -> u32 {
        50 // 处理器优先级
    }
}
```

### 注册自定义处理器

```rust
// 在 main.rs 中注册
channel_manager.register_handler(Arc::new(CustomHandler::new())).await;
```

## 性能优化

### 1. 数据库优化
- 创建适当的索引
- 定期清理过期消息
- 使用连接池

### 2. 内存优化
- 限制队列大小
- 及时处理消息
- 监控内存使用

### 3. 并发优化
- 调整工作线程数
- 使用异步处理
- 避免阻塞操作

## 监控和调试

### 日志级别
- `ERROR`: 错误信息
- `WARN`: 警告信息
- `INFO`: 一般信息
- `DEBUG`: 调试信息

### 关键指标
- 消息处理速度
- 队列长度
- 成功率
- 平均处理时间

## 故障处理

### 常见问题

1. **消息队列满**
   - 检查消费者处理速度
   - 增加工作线程数
   - 扩大队列大小

2. **数据库连接失败**
   - 检查数据库配置
   - 验证网络连接
   - 检查数据库状态

3. **处理器异常**
   - 查看错误日志
   - 检查处理器逻辑
   - 增加错误处理

### 恢复策略
- 自动重试机制
- 消息持久化保证
- 优雅降级处理

## 安全考虑

### 1. 输入验证
- 消息内容长度限制
- 特殊字符过滤
- 格式验证

### 2. 访问控制
- API 认证
- 权限检查
- 速率限制

### 3. 数据保护
- 敏感信息加密
- 访问日志记录
- 数据备份策略

## 未来规划

### 1. 功能增强
- 消息路由规则
- 批量处理支持
- 消息聚合

### 2. 性能提升
- 分布式队列
- 负载均衡
- 缓存优化

### 3. 监控改进
- 实时仪表板
- 告警机制
- 性能分析工具

---

本文档详细介绍了通道层的设计、功能和使用方法。如有问题或建议，请联系开发团队。