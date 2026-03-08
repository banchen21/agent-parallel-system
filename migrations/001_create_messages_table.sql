-- 创建消息表
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- 消息基本信息
    source VARCHAR(20) NOT NULL,           -- 消息来源: api, terminal, internal
    message_type VARCHAR(20) NOT NULL,     -- 消息类型: chat, task, system, query, response
    priority INTEGER NOT NULL DEFAULT 2,   -- 优先级: 1=low, 2=normal, 3=high, 4=critical
    
    -- 发送和接收信息
    sender VARCHAR(255) NOT NULL,          -- 发送者标识
    source_ip VARCHAR(45) NOT NULL,        -- 消息来源IP地址
    recipient VARCHAR(255),                -- 接收者标识（可选，为空则广播）
    content TEXT NOT NULL,                 -- 消息内容
    
    -- 元数据（JSON格式）
    metadata JSONB,                        -- 附加元数据
    
    -- 时间戳
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE, -- 处理时间
    expires_at TIMESTAMP WITH TIME ZONE,   -- 过期时间
    
    -- 处理状态
    status VARCHAR(20) NOT NULL DEFAULT 'queued', -- 处理状态: queued, processing, success, failed, rejected
    error_message TEXT,                    -- 错误信息（如果有）
    retry_count INTEGER NOT NULL DEFAULT 0, -- 重试次数
    
    -- 处理结果
    result_content TEXT,                   -- 处理结果内容
    
    -- 索引
    CONSTRAINT valid_source CHECK (source IN ('api', 'terminal', 'internal')),
    CONSTRAINT valid_message_type CHECK (message_type IN ('chat', 'task', 'system', 'query', 'response')),
    CONSTRAINT valid_priority CHECK (priority BETWEEN 1 AND 4),
    CONSTRAINT valid_status CHECK (status IN ('queued', 'processing', 'success', 'failed', 'rejected'))
);

-- 创建索引以提高查询性能
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender);
CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(recipient);
CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status);
CREATE INDEX IF NOT EXISTS idx_messages_source_ip ON messages(source_ip);
CREATE INDEX IF NOT EXISTS idx_messages_message_type ON messages(message_type);
CREATE INDEX IF NOT EXISTS idx_messages_priority ON messages(priority);
CREATE INDEX IF NOT EXISTS idx_messages_source ON messages(source);

-- 创建复合索引用于常见查询
CREATE INDEX IF NOT EXISTS idx_messages_sender_created_at ON messages(sender, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_status_created_at ON messages(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_type_status ON messages(message_type, status);

-- 创建分区表（按月分区，提高大数据量下的性能）
-- 注意：这需要PostgreSQL 10+
-- 这里先注释掉，可以根据需要启用
/*
CREATE TABLE messages_y2024m01 PARTITION OF messages
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

CREATE TABLE messages_y2024m02 PARTITION OF messages
    FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');
*/

-- 创建触发器，自动更新处理时间
CREATE OR REPLACE FUNCTION update_processed_at()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status IN ('success', 'failed', 'rejected') AND OLD.status != NEW.status THEN
        NEW.processed_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_processed_at
    BEFORE UPDATE ON messages
    FOR EACH ROW
    EXECUTE FUNCTION update_processed_at();

-- 创建视图，用于快速查询最近的聊天记录
CREATE OR REPLACE VIEW recent_chats AS
SELECT 
    id,
    sender,
    recipient,
    content,
    created_at,
    status,
    CASE 
        WHEN status = 'success' THEN result_content
        ELSE NULL
    END as response
FROM messages 
WHERE message_type = 'chat' 
  AND created_at > NOW() - INTERVAL '7 days'
ORDER BY created_at DESC;

-- 创建视图，用于统计消息处理情况
CREATE OR REPLACE VIEW message_stats AS
SELECT 
    DATE(created_at) as date,
    message_type,
    source,
    status,
    COUNT(*) as count,
    AVG(EXTRACT(EPOCH FROM (processed_at - created_at))) as avg_processing_time_seconds
FROM messages 
WHERE created_at >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY DATE(created_at), message_type, source, status
ORDER BY date DESC, count DESC;

-- 插入一些示例数据（可选）
INSERT INTO messages (source, message_type, sender, content, status) VALUES
('api', 'chat', 'user1', '你好，我想了解一下这个系统', 'success'),
('terminal', 'system', 'admin', '/status', 'success'),
('api', 'task', 'user2', '请帮我分析一下数据', 'processing')
ON CONFLICT DO NOTHING;

-- 创建存储过程，用于清理过期消息
CREATE OR REPLACE FUNCTION cleanup_expired_messages()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM messages 
    WHERE expires_at IS NOT NULL 
      AND expires_at < NOW()
      AND status IN ('success', 'failed', 'rejected');
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 创建存储过程，用于获取消息统计
CREATE OR REPLACE FUNCTION get_message_statistics(
    p_days INTEGER DEFAULT 7
)
RETURNS TABLE(
    total_messages BIGINT,
    successful_messages BIGINT,
    failed_messages BIGINT,
    avg_processing_time DECIMAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        COUNT(*) as total_messages,
        COUNT(*) FILTER (WHERE status = 'success') as successful_messages,
        COUNT(*) FILTER (WHERE status = 'failed') as failed_messages,
        AVG(EXTRACT(EPOCH FROM (processed_at - created_at))) as avg_processing_time
    FROM messages 
    WHERE created_at >= NOW() - INTERVAL '1 day' * p_days;
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE messages IS '消息存储表，用于记录所有通过通道层的消息';
COMMENT ON COLUMN messages.id IS '消息唯一标识符';
COMMENT ON COLUMN messages.source IS '消息来源：api、terminal、internal';
COMMENT ON COLUMN messages.message_type IS '消息类型：chat、task、system、query、response';
COMMENT ON COLUMN messages.priority IS '优先级：1=低，2=普通，3=高，4=紧急';
COMMENT ON COLUMN messages.sender IS '发送者标识';
COMMENT ON COLUMN messages.recipient IS '接收者标识，为空表示广播';
COMMENT ON COLUMN messages.content IS '消息内容';
COMMENT ON COLUMN messages.metadata IS 'JSON格式的附加元数据';
COMMENT ON COLUMN messages.status IS '处理状态：queued、processing、success、failed、rejected';
COMMENT ON COLUMN messages.result_content IS '处理结果内容';