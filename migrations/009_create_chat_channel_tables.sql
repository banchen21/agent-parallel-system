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
