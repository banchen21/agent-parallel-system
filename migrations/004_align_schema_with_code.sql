CREATE EXTENSION IF NOT EXISTS pgcrypto;

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

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS is_superuser BOOLEAN DEFAULT false;

ALTER TABLE workspaces
    ADD COLUMN IF NOT EXISTS context JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}'::jsonb;

ALTER TABLE workspaces
    ALTER COLUMN name SET NOT NULL,
    ALTER COLUMN owner_id SET NOT NULL,
    ALTER COLUMN is_public SET NOT NULL,
    ALTER COLUMN context SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL,
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN updated_at SET NOT NULL;

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

CREATE INDEX IF NOT EXISTS idx_workspace_permissions_workspace_id ON workspace_permissions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_workspace_permissions_user_id ON workspace_permissions(user_id);
CREATE INDEX IF NOT EXISTS idx_workspace_permissions_agent_id ON workspace_permissions(agent_id);

ALTER TABLE tasks
    DROP COLUMN IF EXISTS task_type CASCADE,
    DROP COLUMN IF EXISTS input_data CASCADE,
    DROP COLUMN IF EXISTS output_data CASCADE,
    DROP COLUMN IF EXISTS estimated_duration CASCADE,
    DROP COLUMN IF EXISTS actual_duration CASCADE,
    DROP COLUMN IF EXISTS error_message CASCADE,
    DROP COLUMN IF EXISTS max_retries CASCADE;

ALTER TABLE tasks
    ADD COLUMN IF NOT EXISTS requirements JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS context JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS result JSONB,
    ADD COLUMN IF NOT EXISTS progress INTEGER DEFAULT 0,
    ADD COLUMN IF NOT EXISTS current_step TEXT,
    ADD COLUMN IF NOT EXISTS estimated_completion TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS execution_time INTEGER,
    ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}'::jsonb;

ALTER TABLE tasks
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN priority DROP DEFAULT;

ALTER TABLE tasks
    ALTER COLUMN status TYPE task_status
    USING (
        CASE
            WHEN status IS NULL THEN 'pending'
            WHEN lower(status::text) IN ('pending', 'in_progress', 'completed', 'failed', 'cancelled') THEN lower(status::text)
            ELSE 'pending'
        END
    )::task_status;

ALTER TABLE tasks
    ALTER COLUMN priority TYPE task_priority
    USING (
        CASE
            WHEN priority IS NULL THEN 'medium'
            WHEN lower(priority::text) = 'critical' THEN 'urgent'
            WHEN lower(priority::text) IN ('low', 'medium', 'high', 'urgent') THEN lower(priority::text)
            ELSE 'medium'
        END
    )::task_priority;

ALTER TABLE tasks
    ALTER COLUMN status SET DEFAULT 'pending',
    ALTER COLUMN priority SET DEFAULT 'medium';

ALTER TABLE tasks
    ALTER COLUMN title SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ALTER COLUMN priority SET NOT NULL,
    ALTER COLUMN workspace_id SET NOT NULL,
    ALTER COLUMN created_by SET NOT NULL,
    ALTER COLUMN requirements SET NOT NULL,
    ALTER COLUMN context SET NOT NULL,
    ALTER COLUMN progress SET NOT NULL,
    ALTER COLUMN retry_count SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL,
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN updated_at SET NOT NULL;

ALTER TABLE task_dependencies
    ADD COLUMN IF NOT EXISTS id UUID DEFAULT gen_random_uuid();

ALTER TABLE task_dependencies
    ALTER COLUMN id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_dependencies_id ON task_dependencies(id);

ALTER TABLE task_dependencies
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type DROP DEFAULT;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type TYPE dependency_type
    USING (
        CASE
            WHEN dependency_type IS NULL THEN 'blocking'
            WHEN lower(dependency_type::text) = 'hard' THEN 'blocking'
            WHEN lower(dependency_type::text) = 'soft' THEN 'nonblocking'
            WHEN lower(dependency_type::text) IN ('blocking', 'nonblocking') THEN lower(dependency_type::text)
            ELSE 'blocking'
        END
    )::dependency_type;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type SET DEFAULT 'blocking';

ALTER TABLE agents
    DROP COLUMN IF EXISTS agent_type CASCADE,
    DROP COLUMN IF EXISTS configuration CASCADE,
    DROP COLUMN IF EXISTS workspace_id CASCADE,
    DROP COLUMN IF EXISTS created_by CASCADE,
    DROP COLUMN IF EXISTS last_heartbeat_at CASCADE,
    DROP COLUMN IF EXISTS current_task_id CASCADE,
    DROP COLUMN IF EXISTS performance_metrics CASCADE,
    DROP COLUMN IF EXISTS error_count CASCADE;

ALTER TABLE agents
    ADD COLUMN IF NOT EXISTS endpoints JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS limits JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS current_load INTEGER DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_heartbeat TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}'::jsonb;

ALTER TABLE agents
    ALTER COLUMN status DROP DEFAULT;

ALTER TABLE agents
    ALTER COLUMN status TYPE agent_status
    USING (
        CASE
            WHEN status IS NULL THEN 'offline'
            WHEN lower(status::text) = 'active' THEN 'online'
            WHEN lower(status::text) = 'inactive' THEN 'offline'
            WHEN lower(status::text) IN ('online', 'offline', 'busy', 'idle', 'error') THEN lower(status::text)
            ELSE 'offline'
        END
    )::agent_status;

ALTER TABLE agents
    ALTER COLUMN status SET DEFAULT 'offline';

ALTER TABLE agents
    ALTER COLUMN name SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ALTER COLUMN capabilities SET NOT NULL,
    ALTER COLUMN endpoints SET NOT NULL,
    ALTER COLUMN limits SET NOT NULL,
    ALTER COLUMN current_load SET NOT NULL,
    ALTER COLUMN max_concurrent_tasks SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL,
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN updated_at SET NOT NULL;

CREATE TABLE IF NOT EXISTS agent_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    message_type VARCHAR(100) NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS task_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    message_type VARCHAR(100) NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS user_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_type VARCHAR(100) NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_agent_messages_agent_id ON agent_messages(agent_id);
CREATE INDEX IF NOT EXISTS idx_task_messages_task_id ON task_messages(task_id);
CREATE INDEX IF NOT EXISTS idx_user_messages_user_id ON user_messages(user_id);

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
