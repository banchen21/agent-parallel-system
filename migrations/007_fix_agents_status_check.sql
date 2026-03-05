-- Align agents.status constraint with current code values.
-- The code uses: online/offline/busy/idle/error.

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
