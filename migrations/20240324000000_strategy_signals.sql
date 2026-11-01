-- Add new columns
ALTER TABLE strategy ADD COLUMN buy_signal_percent NUMERIC(20, 9);
ALTER TABLE strategy ADD COLUMN buy_active_volatility_threshold NUMERIC(20, 9);
ALTER TABLE strategy ADD COLUMN buy_signal_min_sol_liquidity NUMERIC(20, 9);
ALTER TABLE strategy ADD COLUMN buy_signal_min_volatility NUMERIC(20, 9);

-- Rename stop_loss_percent
ALTER TABLE strategy RENAME COLUMN stop_loss_percent TO sell_signal_stop_loss_percent;

-- Migrate data from params to new columns
UPDATE strategy 
SET buy_signal_percent = (params->>'buy_signal_percentage')::NUMERIC,
    buy_active_volatility_threshold = (params->>'volatility_trigger')::NUMERIC;

-- Remove old fields from params
UPDATE strategy 
SET params = params - 'buy_signal_percentage' - 'volatility_trigger'
WHERE params IS NOT NULL; 