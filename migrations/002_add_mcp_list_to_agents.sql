-- 为 agents 表添加 mcp_list 字段，存储智能体可用的 MCP 工具列表
ALTER TABLE agents ADD COLUMN IF NOT EXISTS mcp_list TEXT[] DEFAULT '{}';

-- 添加注释
COMMENT ON COLUMN agents.mcp_list IS '智能体可用的 MCP 工具列表';
