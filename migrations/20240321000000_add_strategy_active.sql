-- Add is_active column to strategy table
ALTER TABLE strategy ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
CREATE INDEX IF NOT EXISTS idx_strategy_is_active ON strategy(is_active); 