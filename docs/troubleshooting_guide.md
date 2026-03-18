# 故障排查指南

本文档按当前项目的真实问题形态整理。

## 1. 后端起不来

### 排查顺序

1. 执行 `cargo check`
2. 检查 PostgreSQL、Redis、Neo4j 是否可连
3. 检查环境变量
4. 检查端口 8000 是否被占用

## 2. 任务列表为空

常见原因：

1. 后端服务没运行
2. Access Token 失效
3. 当前用户名下没有工作区
4. 查询结果被工作区 owner 过滤掉了

## 3. 任务一直停在 submitted

排查：

1. `TaskAgent` 是否注册成功
2. `DagOrchestrActor` 是否还在运行
3. 日志里是否出现回补扫描记录
4. `task_reviews` 是否一直没有写入

## 4. 任务通知要刷新才看得到

优先检查 WebSocket 订阅链路：

1. `ChatWsSession.started()` 是否执行
2. 是否发出了 `SubscribeChannelNotify`
3. `ChannelEvent.user` 是否是 `任务系统`
4. 前端是否识别 `task_progress`

## 5. MCP 工具建出来但不能执行

检查点：

1. 工具是否带 `execution`
2. `transport` 是否为 `builtin` 或可用 `http`
3. `endpoint` 是否正确
4. 自动生成工具是否具备 `command` 参数

## 6. 配置改了但没生效

原因通常是：

1. 配置接口只会写 `config/default.toml`
2. 运行中的 Actor 不会热更新

处理方式：重启后端服务。
