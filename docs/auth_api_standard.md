# 用户认证 API 接口标准

## 概述

本文档定义了 agent-parallel-system 项目的用户认证相关 API 接口标准，包括用户注册、登录、令牌刷新、密码重置等功能。

## 认证流程

```mermaid
graph TD
    A[用户注册] --> B[用户登录]
    B --> C[获取访问令牌]
    C --> D[访问受保护资源]
    D --> E[令牌过期]
    E --> F[刷新令牌]
    F --> D
```

## API 端点列表

| 方法 | 路径 | 描述 | 认证要求 |
|------|------|------|----------|
| POST | `/api/auth/register` | 用户注册 | 无需认证 |
| POST | `/api/auth/login` | 用户登录 | 无需认证 |
| POST | `/api/auth/refresh` | 刷新访问令牌 | 需要刷新令牌 |
| POST | `/api/auth/logout` | 用户登出 | 需要访问令牌 |
| POST | `/api/auth/password-reset` | 请求密码重置 | 无需认证 |
| PUT | `/api/auth/password-reset` | 重置密码 | 需要重置令牌 |
| GET | `/api/auth/profile` | 获取用户信息 | 需要访问令牌 |
| PUT | `/api/auth/profile` | 更新用户信息 | 需要访问令牌 |

## 请求和响应格式

### 1. 用户注册

**请求**
```json
POST /api/auth/register
Content-Type: application/json

{
  "username": "string",
  "email": "string",
  "password": "string",
  "display_name": "string"
}
```

**字段说明**
| 字段名 | 类型 | 必需 | 说明 |
|--------|------|------|------|
| username | string | 是 | 用户名（3-50个字符，只能包含字母、数字、下划线） |
| email | string | 是 | 邮箱地址 |
| password | string | 是 | 密码（至少8个字符，包含大小写字母和数字） |
| display_name | string | 否 | 显示名称 |

**响应**
```json
{
  "success": true,
  "message": "用户注册成功",
  "data": {
    "user_id": "uuid",
    "username": "string",
    "email": "string",
    "display_name": "string",
    "created_at": "2024-01-01T00:00:00Z"
  }
}
```

**错误响应**
```json
{
  "success": false,
  "error": {
    "code": "USERNAME_EXISTS",
    "message": "用户名已存在"
  }
}
```

### 2. 用户登录

**请求**
```json
POST /api/auth/login
Content-Type: application/json

{
  "username": "string",
  "password": "string"
}
```

**字段说明**
| 字段名 | 类型 | 必需 | 说明 |
|--------|------|------|------|
| username | string | 是 | 用户名或邮箱 |
| password | string | 是 | 密码 |

**响应**
```json
{
  "success": true,
  "message": "登录成功",
  "data": {
    "access_token": "string",
    "refresh_token": "string",
    "token_type": "Bearer",
    "expires_in": 3600,
    "user": {
      "user_id": "uuid",
      "username": "string",
      "email": "string",
      "display_name": "string",
      "role": "user"
    }
  }
}
```

### 3. 刷新访问令牌

**请求**
```json
POST /api/auth/refresh
Content-Type: application/json
Authorization: Bearer {refresh_token}

{
  "refresh_token": "string"
}
```

**响应**
```json
{
  "success": true,
  "message": "令牌刷新成功",
  "data": {
    "access_token": "string",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### 4. 用户登出

**请求**
```json
POST /api/auth/logout
Content-Type: application/json
Authorization: Bearer {access_token}
```

**响应**
```json
{
  "success": true,
  "message": "登出成功"
}
```

### 5. 请求密码重置

**请求**
```json
POST /api/auth/password-reset
Content-Type: application/json

{
  "email": "string"
}
```

**响应**
```json
{
  "success": true,
  "message": "密码重置链接已发送到您的邮箱"
}
```

### 6. 重置密码

**请求**
```json
PUT /api/auth/password-reset
Content-Type: application/json

{
  "token": "string",
  "new_password": "string"
}
```

**响应**
```json
{
  "success": true,
  "message": "密码重置成功"
}
```

### 7. 获取用户信息

**请求**
```json
GET /api/auth/profile
Authorization: Bearer {access_token}
```

**响应**
```json
{
  "success": true,
  "data": {
    "user_id": "uuid",
    "username": "string",
    "email": "string",
    "display_name": "string",
    "role": "user",
    "created_at": "2024-01-01T00:00:00Z",
    "last_login": "2024-01-01T00:00:00Z"
  }
}
```

### 8. 更新用户信息

**请求**
```json
PUT /api/auth/profile
Content-Type: application/json
Authorization: Bearer {access_token}

{
  "display_name": "string",
  "email": "string"
}
```

**响应**
```json
{
  "success": true,
  "message": "用户信息更新成功",
  "data": {
    "user_id": "uuid",
    "username": "string",
    "email": "string",
    "display_name": "string"
  }
}
```

## 错误码定义

| 错误码 | HTTP状态码 | 描述 |
|--------|------------|------|
| INVALID_CREDENTIALS | 401 | 用户名或密码错误 |
| USERNAME_EXISTS | 409 | 用户名已存在 |
| EMAIL_EXISTS | 409 | 邮箱已存在 |
| INVALID_TOKEN | 401 | 无效的令牌 |
| TOKEN_EXPIRED | 401 | 令牌已过期 |
| INSUFFICIENT_PERMISSIONS | 403 | 权限不足 |
| USER_NOT_FOUND | 404 | 用户不存在 |
| INVALID_PASSWORD_FORMAT | 400 | 密码格式不符合要求 |
| RATE_LIMIT_EXCEEDED | 429 | 请求频率过高 |

## 安全要求

### 密码安全
- 密码必须使用 bcrypt 或 argon2 进行哈希存储
- 密码最小长度：8个字符
- 必须包含大小写字母和数字
- 禁止使用常见弱密码

### 令牌安全
- 访问令牌有效期：1小时
- 刷新令牌有效期：7天
- 使用 JWT (JSON Web Token) 格式
- 令牌必须包含用户ID和角色信息
- 刷新令牌必须存储在安全的 HTTP-only cookie 中

### 速率限制
- 登录尝试：5次/分钟
- 注册请求：3次/分钟
- 密码重置请求：2次/小时

## 数据库表结构

### users 表
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(100),
    role VARCHAR(20) NOT NULL DEFAULT 'user',
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_login TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
```

### refresh_tokens 表
```sql
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    is_revoked BOOLEAN NOT NULL DEFAULT false
);
```

### password_reset_tokens 表
```sql
CREATE TABLE password_reset_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
```

## 环境变量配置

```bash
# JWT 配置
JWT_SECRET=your-jwt-secret-key
JWT_ACCESS_EXPIRES_IN=3600
JWT_REFRESH_EXPIRES_IN=604800

# 密码哈希配置
BCRYPT_COST=12

# 邮件配置（用于密码重置）
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=your-email@example.com
SMTP_PASSWORD=your-email-password
```

## 示例代码

### 注册请求示例
```bash
curl -X POST http://localhost:8080/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com", 
    "password": "Test123456",
    "display_name": "测试用户"
  }'
```

### 登录请求示例
```bash
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "Test123456"
  }'
```

### 访问受保护资源示例
```bash
curl -X GET http://localhost:8080/api/auth/profile \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."