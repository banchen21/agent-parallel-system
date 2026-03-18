# Channel Layer 说明

当前项目的通道层由 `ChannelManagerActor` 驱动，承担三类职责：

1. 聊天消息持久化
2. 历史消息查询
3. 实时广播任务系统通知到 WebSocket 会话

## 当前实现位置

核心代码位于：

1. `src/channel/actor_messages.rs`
2. `src/chat/handler.rs`
3. `src/chat/ws_handler.rs`

## 设计目标

它主要服务两个场景：

1. 聊天历史记录的数据库读写
2. 任务系统通知的实时推送

## 主要消息

实际会用到的消息类型包括：

1. `SaveMessage`
2. `GetMessages`
3. `SubscribeChannelNotify`
4. `UnsubscribeChannelNotify`
5. `ChannelEvent`

## 聊天写入链路

1. 用户消息进入 ChatAgent 或 WebSocket 会话
2. 构造 `UserMessage`
3. 发送 `SaveMessage`
4. ChannelManagerActor 写入 PostgreSQL

## 历史查询链路

`GET /api/v1/message` 通过 `GetMessages` 获取聊天历史。

## 实时广播链路

1. WebSocket 建立时，`ChatWsSession.started()` 会向 ChannelManager 注册订阅
2. WebSocket 关闭时，会取消订阅
3. 当通道层广播 `ChannelEvent` 时，所有订阅会话都会收到
4. `ChatWsSession` 会过滤 `msg.user == "任务系统"` 的通知
5. 符合条件的消息会以 `task_progress` 类型推送给前端

## 当前边界

通道层当前并不负责：

1. 通用事件系统
2. 多主题消息路由
3. 任意类型消息插件化处理
