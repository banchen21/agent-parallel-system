# 认证 API 说明

本文档只描述当前项目已经实现并注册到路由中的认证接口。

## 路由范围

公开接口都在 `/auth` 作用域下，不经过 `Auth` 中间件：

1. `POST /auth/register`
2. `POST /auth/login`
3. `POST /auth/refresh`

## Token 模型

当前系统使用双 Token：

1. Access Token：用于访问 `/api/v1/*` 和 `ws/chat`、`logs/stream`
2. Refresh Token：用于换发新的 Token 对

Refresh Token 会写入 Redis，并通过一次性消费机制实现轮换。

## 1. 注册

- 方法：`POST`
- 路径：`/auth/register`

请求体：

```json
{
  "username": "banchen",
  "password": "123456",
  "email": "banchen@example.com"
}
```

行为：

1. 使用 bcrypt 加密密码
2. 创建用户记录
3. 异步创建默认工作区
4. 异步创建默认 Agent

## 2. 登录

- 方法：`POST`
- 路径：`/auth/login`

请求体：

```json
{
  "username": "banchen",
  "password": "123456"
}
```

成功返回：

```json
{
  "access_token": "...",
  "refresh_token": "..."
}
```

失败场景：

1. 用户不存在：`401 Unauthorized`
2. 密码错误：`401 Unauthorized`
3. Redis 写入 Refresh Token 失败：`500 Internal Server Error`

## 3. 刷新 Token

- 方法：`POST`
- 路径：`/auth/refresh`

请求头：

```text
Authorization: Bearer <refresh_token>
```

行为：

1. 从 `Authorization` 头读取 Refresh Token
2. 本地验证 JWT
3. 在 Redis 中验证并消费旧 Refresh Token
4. 生成新的 Access Token 和 Refresh Token
5. 将新的 Refresh Token 再写回 Redis

## 受保护接口认证方式

`/api/v1/*` 接口统一使用：

```text
Authorization: Bearer <access_token>
```

WebSocket 和 SSE 使用查询参数：

1. `GET /ws/chat?token=<access_token>`
2. `GET /logs/stream?token=<access_token>`

## 当前未实现的接口

以下能力没有在当前代码中注册为可用路由：

1. logout
2. password reset
3. profile 查询与修改
