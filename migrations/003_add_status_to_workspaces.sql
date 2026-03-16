-- Add status column to workspaces for workspace lifecycle
ALTER TABLE workspaces
ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'active';

-- Optional: normalize existing rows (set to 'active' when NULL)
UPDATE workspaces SET status = 'active' WHERE status IS NULL;

-- Create index for status if queries will filter by it
CREATE INDEX IF NOT EXISTS idx_workspaces_status ON workspaces (status);
