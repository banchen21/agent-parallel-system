# 数据库架构文档

本文档整理了 Agent Parallel System 项目的所有数据库迁移脚本。

## 目录

1. [001 - 创建用户表](#001-创建用户表)
2. [002 - 创建工作空间表](#002-创建工作空间表)
3. [003 - 创建智能体和消息表](#003-创建智能体和消息表)
4. [004 - 对齐架构与代码](#004-对齐架构与代码)
5. [005 - 转换枚举列为 VARCHAR](#005-转换枚举列为-varchar)
6. [006 - 强化用户列约束](#006-强化用户列约束)
7. [007 - 修复智能体状态检查](#007-修复智能体状态检查)
8. [008 - 创建工作流表](#008-创建工作流表)
9. [009 - 创建聊天和通道表](#009-创建聊天和通道表)

---

## 001 创建用户表

**文件**: `001_create_users_table.sql`

```sql
-- 创建用户表
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    role VARCHAR(20) DEFAULT 'user' CHECK (role IN ('user', 'admin', 'super_admin')),
    is_active BOOLEAN DEFAULT true,
    last_login_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建用户表的索引
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);

-- 创建用户会话表
CREATE TABLE IF NOT EXISTS user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    refresh_token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    refresh_expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    user_agent TEXT,
    ip_address INET,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建用户会话表的索引
CREATE INDEX IF NOT EXISTS idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_sessions_token_hash ON user_sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires_at ON user_sessions(expires_at);
```

---

## 002 创建工作空间表

**文件**: `002_create_workspaces_tables.sql`

```sql
-- 创建工作空间表
CREATE TABLE IF NOT EXISTS workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_public BOOLEAN DEFAULT false,
    settings JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建工作空间成员表
CREATE TABLE IF NOT EXISTS workspace_members (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    joined_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (workspace_id, user_id)
);

-- 创建任务表
CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(20) DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed', 'failed', 'cancelled')),
    priority VARCHAR(10) DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    task_type VARCHAR(50) NOT NULL,
    input_data JSONB DEFAULT '{}'::jsonb,
    output_data JSONB DEFAULT '{}'::jsonb,
    metadata JSONB DEFAULT '{}'::jsonb,
    assigned_agent_id UUID,
    parent_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    estimated_duration INTEGER, -- 单位：秒
    actual_duration INTEGER, -- 单位：秒
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建任务依赖表
CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    dependency_type VARCHAR(20) DEFAULT 'hard' CHECK (dependency_type IN ('hard', 'soft')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (task_id, depends_on_task_id)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_workspaces_owner_id ON workspaces(owner_id);
CREATE INDEX IF NOT EXISTS idx_workspaces_created_at ON workspaces(created_at);
CREATE INDEX IF NOT EXISTS idx_workspace_members_user_id ON workspace_members(user_id);
CREATE INDEX IF NOT EXISTS idx_tasks_workspace_id ON tasks(workspace_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned_agent_id ON tasks(assigned_agent_id);
CREATE INDEX IF NOT EXISTS idx_tasks_created_by ON tasks(created_by);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_completed_at ON tasks(completed_at);
CREATE INDEX IF NOT EXISTS idx_task_dependencies_task_id ON task_dependencies(task_id);
CREATE INDEX IF NOT EXISTS idx_task_dependencies_depends_on_task_id ON task_dependencies(depends_on_task_id);
```

---

## 003 创建智能体和消息表

**文件**: `003_create_agents_messages_tables.sql`

```sql
-- 创建智能体表
CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    agent_type VARCHAR(50) NOT NULL,
    capabilities JSONB DEFAULT '[]'::jsonb,
    configuration JSONB DEFAULT '{}'::jsonb,
    status VARCHAR(20) DEFAULT 'inactive' CHECK (status IN ('active', 'inactive', 'busy', 'error')),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_heartbeat_at TIMESTAMP WITH TIME ZONE,
    current_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    performance_metrics JSONB DEFAULT '{}'::jsonb,
    error_count INTEGER DEFAULT 0,
    max_concurrent_tasks INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建智能体分配表
CREATE TABLE IF NOT EXISTS agent_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20) DEFAULT 'assigned' CHECK (status IN ('assigned', 'in_progress', 'completed', 'failed')),
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    UNIQUE (agent_id, task_id)
);

-- 创建消息表
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    sender_type VARCHAR(20) NOT NULL CHECK (sender_type IN ('user', 'agent', 'system')),
    sender_id UUID NOT NULL, -- 可以是user_id或agent_id
    receiver_type VARCHAR(20) NOT NULL CHECK (receiver_type IN ('user', 'agent', 'system', 'broadcast')),
    receiver_id UUID, -- 可以是user_id或agent_id，broadcast时为NULL
    message_type VARCHAR(50) NOT NULL,
    content JSONB NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb,
    is_read BOOLEAN DEFAULT false,
    read_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建任务执行日志表
CREATE TABLE IF NOT EXISTS task_execution_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    log_level VARCHAR(10) DEFAULT 'info' CHECK (log_level IN ('debug', 'info', 'warn', 'error')),
    message TEXT NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_agents_workspace_id ON agents(workspace_id);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
CREATE INDEX IF NOT EXISTS idx_agents_agent_type ON agents(agent_type);
CREATE INDEX IF NOT EXISTS idx_agents_created_by ON agents(created_by);
CREATE INDEX IF NOT EXISTS idx_agent_assignments_agent_id ON agent_assignments(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_assignments_task_id ON agent_assignments(task_id);
CREATE INDEX IF NOT EXISTS idx_agent_assignments_status ON agent_assignments(status);
CREATE INDEX IF NOT EXISTS idx_messages_workspace_id ON messages(workspace_id);
CREATE INDEX IF NOT EXISTS idx_messages_sender_id ON messages(sender_id);
CREATE INDEX IF NOT EXISTS idx_messages_receiver_id ON messages(receiver_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
CREATE INDEX IF NOT EXISTS idx_messages_is_read ON messages(is_read);
CREATE INDEX IF NOT EXISTS idx_task_execution_logs_task_id ON task_execution_logs(task_id);
CREATE INDEX IF NOT EXISTS idx_task_execution_logs_agent_id ON task_execution_logs(agent_id);
CREATE INDEX IF NOT EXISTS idx_task_execution_logs_created_at ON task_execution_logs(created_at);

-- 更新tasks表的外键约束
ALTER TABLE tasks 
    ADD CONSTRAINT fk_tasks_assigned_agent_id 
    FOREIGN KEY (assigned_agent_id) REFERENCES agents(id) ON DELETE SET NULL;
```

---

## 004 对齐架构与代码

**文件**: `004_align_schema_with_code.sql`

此迁移脚本创建了枚举类型、添加了额外的列和表，以及建立了更多的索引。由于内容较长，这里展示关键部分：

```sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- 创建枚举类型
DO $$
BEGIN
    CREATE TYPE task_status AS ENUM ('pending', 'in_progress', 'completed', 'failed', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    CREATE TYPE task_priority AS ENUM ('low', 'medium', 'high', 'urgent');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    CREATE TYPE agent_status AS ENUM ('online', 'offline', 'busy', 'idle', 'error');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    CREATE TYPE permission_level AS ENUM ('read', 'write', 'admin');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    CREATE TYPE dependency_type AS ENUM ('blocking', 'nonblocking');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- 添加用户超级管理员字段
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS is_superuser BOOLEAN DEFAULT false;

-- 更新工作空间表
ALTER TABLE workspaces
    ADD COLUMN IF NOT EXISTS context JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}'::jsonb;

-- 创建工作空间权限表
CREATE TABLE IF NOT EXISTS workspace_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
    permission_level permission_level NOT NULL,
    granted_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    granted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP WITH TIME ZONE
);

-- 更新任务表结构
ALTER TABLE tasks
    ADD COLUMN IF NOT EXISTS requirements JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS result JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS timeout_seconds INTEGER,
    ADD COLUMN IF NOT EXISTS tags TEXT[] DEFAULT ARRAY[]::TEXT[];

-- 创建消息相关表
CREATE TABLE IF NOT EXISTS agent_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    message_type VARCHAR(50) NOT NULL DEFAULT 'text',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS task_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    message_type VARCHAR(50) NOT NULL DEFAULT 'text',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS user_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    message_type VARCHAR(50) NOT NULL DEFAULT 'text',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read BOOLEAN NOT NULL DEFAULT false
);

-- 创建文档表
CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    file_name VARCHAR(255),
    file_type VARCHAR(100),
    file_size BIGINT,
    storage_url TEXT,
    content_type VARCHAR(100),
    content_hash VARCHAR(255),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建工具表
CREATE TABLE IF NOT EXISTS tools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(100),
    endpoint_url TEXT,
    authentication_type VARCHAR(50) NOT NULL DEFAULT 'none',
    authentication_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    parameters_schema JSONB NOT NULL DEFAULT '{}'::jsonb,
    capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

---

## 005 转换枚举列为 VARCHAR

**文件**: `005_convert_enum_columns_to_varchar.sql`

```sql
-- 转换任务状态和优先级
ALTER TABLE tasks
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN priority DROP DEFAULT;

ALTER TABLE tasks
    ALTER COLUMN status TYPE VARCHAR(32) USING status::text,
    ALTER COLUMN priority TYPE VARCHAR(16) USING priority::text;

ALTER TABLE tasks
    ALTER COLUMN status SET DEFAULT 'pending',
    ALTER COLUMN priority SET DEFAULT 'medium',
    ALTER COLUMN status SET NOT NULL,
    ALTER COLUMN priority SET NOT NULL;

-- 转换智能体状态
ALTER TABLE agents
    ALTER COLUMN status DROP DEFAULT;

ALTER TABLE agents
    ALTER COLUMN status TYPE VARCHAR(32) USING status::text;

ALTER TABLE agents
    ALTER COLUMN status SET DEFAULT 'offline',
    ALTER COLUMN status SET NOT NULL;

-- 转换权限级别
ALTER TABLE workspace_permissions
    ALTER COLUMN permission_level TYPE VARCHAR(16) USING permission_level::text,
    ALTER COLUMN permission_level SET NOT NULL;

-- 转换依赖类型
ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type DROP DEFAULT;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type TYPE VARCHAR(32) USING dependency_type::text;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type SET DEFAULT 'blocking';

-- 删除工作空间设置列
ALTER TABLE workspaces
    DROP COLUMN IF EXISTS settings;
```

---

## 006 强化用户列约束

**文件**: `006_harden_users_columns.sql`

```sql
ALTER TABLE users
    ALTER COLUMN role SET DEFAULT 'user',
    ALTER COLUMN role SET NOT NULL,
    ALTER COLUMN is_superuser SET DEFAULT false,
    ALTER COLUMN is_superuser SET NOT NULL,
    ALTER COLUMN is_active SET DEFAULT true,
    ALTER COLUMN is_active SET NOT NULL,
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET NOT NULL;
```

---

## 007 修复智能体状态检查

**文件**: `007_fix_agents_status_check.sql`

```sql
-- 对齐智能体状态约束与代码值
-- 代码使用: online/offline/busy/idle/error

UPDATE agents
SET status = 'online'
WHERE status = 'active';

UPDATE agents
SET status = 'offline'
WHERE status = 'inactive';

ALTER TABLE agents
    DROP CONSTRAINT IF EXISTS agents_status_check;

ALTER TABLE agents
    ADD CONSTRAINT agents_status_check
    CHECK (
        status IN ('online', 'offline', 'busy', 'idle', 'error')
    );
```

---

## 008 创建工作流表

**文件**: `008_create_workflow_tables.sql`

```sql
CREATE TABLE IF NOT EXISTS workflows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    definition JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS workflow_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    triggered_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    options JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(20) NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    result JSONB,
    error_message TEXT,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_workflows_workspace_id ON workflows(workspace_id);
CREATE INDEX IF NOT EXISTS idx_workflows_created_by ON workflows(created_by);
CREATE INDEX IF NOT EXISTS idx_workflows_updated_at ON workflows(updated_at);

CREATE INDEX IF NOT EXISTS idx_workflow_executions_workflow_id ON workflow_executions(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_triggered_by ON workflow_executions(triggered_by);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_task_id ON workflow_executions(task_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_created_at ON workflow_executions(created_at);
```

---

## 009 创建聊天和通道表

**文件**: `009_create_chat_channel_tables.sql`

```sql
-- 创建通道配置表
CREATE TABLE IF NOT EXISTS channel_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type VARCHAR(50) NOT NULL CHECK (channel_type IN ('telegram', 'discord', 'qq', 'web')),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    config JSONB NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(channel_type, name)
);

-- 创建通道用户映射表
CREATE TABLE IF NOT EXISTS channel_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_config_id UUID NOT NULL REFERENCES channel_configs(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    channel_user_id VARCHAR(255) NOT NULL,
    channel_username VARCHAR(255),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(channel_config_id, channel_user_id)
);

-- 创建聊天会话表
CREATE TABLE IF NOT EXISTS chat_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_user_id UUID NOT NULL REFERENCES channel_users(id) ON DELETE CASCADE,
    title VARCHAR(255),
    model VARCHAR(100) NOT NULL DEFAULT 'gpt-3.5-turbo',
    system_prompt TEXT,
    temperature FLOAT DEFAULT 0.7,
    max_tokens INTEGER DEFAULT 2000,
    context_window INTEGER DEFAULT 10,
    metadata JSONB DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建聊天消息表
CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    tokens_used INTEGER,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建通道消息日志表
CREATE TABLE IF NOT EXISTS channel_message_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_config_id UUID NOT NULL REFERENCES channel_configs(id) ON DELETE CASCADE,
    channel_message_id VARCHAR(255),
    channel_user_id VARCHAR(255) NOT NULL,
    message_type VARCHAR(50) NOT NULL CHECK (message_type IN ('text', 'image', 'file', 'command')),
    content TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'received' CHECK (status IN ('received', 'processing', 'sent', 'failed')),
    error_message TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建 LLM 配置表
CREATE TABLE IF NOT EXISTS llm_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    provider VARCHAR(100) NOT NULL CHECK (provider IN ('openai', 'ollama', 'local')),
    api_endpoint TEXT NOT NULL,
    api_key TEXT,
    model_name VARCHAR(255) NOT NULL,
    temperature FLOAT DEFAULT 0.7,
    max_tokens INTEGER DEFAULT 2000,
    is_default BOOLEAN DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_channel_configs_type ON channel_configs(channel_type);
CREATE INDEX IF NOT EXISTS idx_channel_configs_active ON channel_configs(is_active);
CREATE INDEX IF NOT EXISTS idx_channel_users_channel_config ON channel_users(channel_config_id);
CREATE INDEX IF NOT EXISTS idx_channel_users_user_id ON channel_users(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_channel_user ON chat_sessions(channel_user_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_active ON chat_sessions(is_active);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_created ON chat_messages(created_at);
CREATE INDEX IF NOT EXISTS idx_channel_message_logs_config ON channel_message_logs(channel_config_id);
CREATE INDEX IF NOT EXISTS idx_channel_message_logs_created ON channel_message_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_llm_configs_default ON llm_configs(is_default);
CREATE INDEX IF NOT EXISTS idx_llm_configs_active ON llm_configs(is_active);
```

---

## 数据库架构总览

### 核心表

1. **users** - 用户表
2. **user_sessions** - 用户会话表
3. **workspaces** - 工作空间表
4. **workspace_members** - 工作空间成员表
5. **workspace_permissions** - 工作空间权限表
6. **tasks** - 任务表
7. **task_dependencies** - 任务依赖表
8. **agents** - 智能体表
9. **agent_assignments** - 智能体分配表
10. **messages** - 消息表
11. **task_execution_logs** - 任务执行日志表

### 扩展表

12. **workflows** - 工作流表
13. **workflow_executions** - 工作流执行表
14. **channel_configs** - 通道配置表
15. **channel_users** - 通道用户映射表
16. **chat_sessions** - 聊天会话表
17. **chat_messages** - 聊天消息表
18. **channel_message_logs** - 通道消息日志表
19. **llm_configs** - LLM 配置表
20. **documents** - 文档表
21. **tools** - 工具表
22. **agent_messages** - 智能体消息表
23. **task_messages** - 任务消息表
24. **user_messages** - 用户消息表

### 关键关系

- 用户 → 工作空间（所有者）
- 工作空间 → 任务
- 任务 → 智能体（分配）
- 任务 → 任务（依赖关系）
- 工作流 → 任务（执行）
- 通道 → 聊天会话 → 消息

---

**生成时间**: 2026-03-06  
**项目**: Agent Parallel System  
**数据库**: PostgreSQL
