-- Add explicit is_active flag instead of relying on ORDER BY updated_at.
-- Only one session should have is_active = 1 at any time.

ALTER TABLE sessions ADD COLUMN is_active INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_sessions_is_active ON sessions(is_active) WHERE is_active = 1;
