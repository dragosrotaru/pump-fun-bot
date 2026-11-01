-- Modify prices table columns
ALTER TABLE prices
    ALTER COLUMN sol_liquidity TYPE NUMERIC(36, 18),
    ALTER COLUMN token_liquidity TYPE NUMERIC(36, 18),
    ALTER COLUMN price_sol TYPE NUMERIC(36, 18);

-- Modify trades table columns
ALTER TABLE trades
    ALTER COLUMN price TYPE NUMERIC(36, 18),
    ALTER COLUMN quantity TYPE NUMERIC(36, 18);

-- Modify outcomes table columns
ALTER TABLE outcomes
    ALTER COLUMN margin_sol TYPE NUMERIC(36, 18),
    ALTER COLUMN roi_percent TYPE NUMERIC(10, 4);

-- Modify strategy table columns
ALTER TABLE strategy
    ALTER COLUMN buy_amount TYPE NUMERIC(36, 18);