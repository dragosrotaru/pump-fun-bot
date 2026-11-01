-- Add stop_loss_percent column to strategy table
ALTER TABLE strategy ADD COLUMN stop_loss_percent NUMERIC(20, 9) DEFAULT NULL; 