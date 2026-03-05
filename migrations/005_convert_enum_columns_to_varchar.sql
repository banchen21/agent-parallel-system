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

ALTER TABLE agents
    ALTER COLUMN status DROP DEFAULT;

ALTER TABLE agents
    ALTER COLUMN status TYPE VARCHAR(32) USING status::text;

ALTER TABLE agents
    ALTER COLUMN status SET DEFAULT 'offline',
    ALTER COLUMN status SET NOT NULL;

ALTER TABLE workspace_permissions
    ALTER COLUMN permission_level TYPE VARCHAR(16) USING permission_level::text,
    ALTER COLUMN permission_level SET NOT NULL;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type DROP DEFAULT;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type TYPE VARCHAR(32) USING dependency_type::text;

ALTER TABLE task_dependencies
    ALTER COLUMN dependency_type SET DEFAULT 'blocking';

ALTER TABLE workspaces
    DROP COLUMN IF EXISTS settings;
