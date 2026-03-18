# Chat Agent 架构

本文档描述当前项目里已经落地的聊天架构。

## 核心 Actor

聊天相关的核心 Actor 有：

1. `ChatAgent`
2. `OpenAIProxyActor`
3. `TaskAgent`
4. `ChannelManagerActor`
5. `AgentMemoryActor`

## 角色分工

### ChatAgent

1. 接收普通对话输入
2. 根据提示词组织上下文
3. 必要时调用 TaskAgent 做任务识别
4. 调用 OpenAI 兼容接口完成推理
5. 返回面向前端的回复内容

### TaskAgent

1. 判断用户消息是不是任务
2. 将复杂请求拆成任务列表
3. 在执行完成后承担审阅代理角色

### OpenAIProxyActor

1. 统一封装多 Provider 模型调用
2. 管理 provider 和 model 配置
3. 为 ChatAgent、TaskAgent、McpAgentActor 提供统一推理出口

### AgentMemoryActor

1. 管理 Neo4j 图记忆
2. 提供记忆查询和图节点操作
3. 提供 AI 名称等上下文能力

### ChannelManagerActor

1. 持久化消息
2. 查询历史消息
3. 广播任务系统通知

## 接入方式

当前聊天系统支持两种接入方式：

1. WebSocket：主通道，用于实时消息和任务通知
2. HTTP：仅保留历史记录查询接口

当前代码没有注册 `POST /api/v1/message`，所以 HTTP 发送消息不是在线能力。

## 与任务系统的关系

1. 用户在聊天里发出复杂执行请求
2. TaskAgent 可将其识别为任务
3. 任务执行过程中的状态变化和审阅提示会再次回流到聊天 WebSocket
