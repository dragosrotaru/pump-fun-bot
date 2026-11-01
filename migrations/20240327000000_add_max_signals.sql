-- add buy_signal_max_percent, buy_signal_max_sol_liquidity, buy_signal_max_volatility
ALTER TABLE strategy ADD COLUMN buy_signal_max_percent float8;
ALTER TABLE strategy ADD COLUMN buy_signal_max_sol_liquidity float8;
ALTER TABLE strategy ADD COLUMN buy_signal_max_volatility float8;
-- rename buy_signal_percent to buy_signal_min_percent
ALTER TABLE strategy RENAME COLUMN buy_signal_percent TO buy_signal_min_percent;
