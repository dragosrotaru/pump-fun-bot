-- Create strategy table
CREATE TABLE IF NOT EXISTS strategy (
    id BIGSERIAL PRIMARY KEY,
    buy_amount NUMERIC(20, 9) NOT NULL,
    sell_slippage NUMERIC(10, 9) NOT NULL,
    buy_slippage NUMERIC(10, 9) NOT NULL,
    sell_compute_units INTEGER NOT NULL,
    buy_compute_units INTEGER NOT NULL,
    sell_compute_price_mlamports BIGINT NOT NULL,
    buy_compute_price_mlamports BIGINT NOT NULL,

    buy_loop_max_inactive_time_ms INTEGER NOT NULL,
    buy_loop_active_interval_ms INTEGER NOT NULL,
    buy_loop_inactive_interval_ms INTEGER NOT NULL,
    
    sell_loop_time_ms INTEGER NOT NULL,
    sell_max_time_ms INTEGER NOT NULL,
    sell_signal_percent NUMERIC(10, 9) NOT NULL,
    type VARCHAR NOT NULL,
    params JSONB
);

-- Create wallets table
CREATE TABLE IF NOT EXISTS wallets (
    wallet_addr VARCHAR PRIMARY KEY
);

-- Create tokens table
CREATE TABLE IF NOT EXISTS tokens (
    token_addr VARCHAR PRIMARY KEY,
    symbol VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    user_addr VARCHAR NOT NULL REFERENCES wallets(wallet_addr),
    bond_curve_addr VARCHAR NOT NULL,
    assoc_bond_curve_addr VARCHAR NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create prices table
CREATE TABLE IF NOT EXISTS prices (
    id BIGSERIAL PRIMARY KEY,
    token_addr VARCHAR NOT NULL REFERENCES tokens(token_addr),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sol_liquidity NUMERIC(20, 9) NOT NULL,
    token_liquidity NUMERIC(20, 9) NOT NULL,
    price_sol NUMERIC(20, 9) NOT NULL
);

-- Create trades table
CREATE TABLE IF NOT EXISTS trades (
    trx_hash VARCHAR PRIMARY KEY,
    token_addr VARCHAR NOT NULL REFERENCES tokens(token_addr),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    action VARCHAR NOT NULL CHECK (action IN ('buy', 'sell')),
    is_simulated BOOLEAN NOT NULL DEFAULT FALSE,
    corr_id UUID NOT NULL,
    price NUMERIC(20, 9) NOT NULL,
    quantity NUMERIC(20, 9) NOT NULL
);

-- Create timings table
CREATE TABLE IF NOT EXISTS timings (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_micros BIGINT NOT NULL,
    action VARCHAR NOT NULL,
    corr_id UUID NOT NULL
);

-- Create errors table
CREATE TABLE IF NOT EXISTS errors (
    id BIGSERIAL PRIMARY KEY,
    corr_id UUID NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    context JSONB,
    message TEXT NOT NULL
);

-- Create outcomes table
CREATE TABLE IF NOT EXISTS outcomes (
    id BIGSERIAL PRIMARY KEY,
    corr_id UUID NOT NULL,
    token_addr VARCHAR NOT NULL REFERENCES tokens(token_addr),
    buy_trade_hash VARCHAR NOT NULL REFERENCES trades(trx_hash),
    sell_trade_hash VARCHAR NOT NULL REFERENCES trades(trx_hash),
    duration_seconds INTEGER NOT NULL,
    margin_sol NUMERIC(20, 9) NOT NULL,
    roi_percent NUMERIC(10, 9) NOT NULL,
    is_simulated BOOLEAN NOT NULL DEFAULT FALSE,
    strategy_id BIGINT NOT NULL REFERENCES strategy(id)
);

-- Create skipped table
CREATE TABLE IF NOT EXISTS skipped (
    id BIGSERIAL PRIMARY KEY,
    corr_id UUID NOT NULL,
    token_addr VARCHAR NOT NULL REFERENCES tokens(token_addr),
    duration_seconds INTEGER NOT NULL,
    is_simulated BOOLEAN NOT NULL DEFAULT FALSE,
    strategy_id BIGINT NOT NULL REFERENCES strategy(id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_tokens_user_addr ON tokens(user_addr);
CREATE INDEX IF NOT EXISTS idx_prices_token_addr_timestamp ON prices(token_addr, timestamp);
CREATE INDEX IF NOT EXISTS idx_trades_token_addr_timestamp ON trades(token_addr, timestamp);
CREATE INDEX IF NOT EXISTS idx_trades_corr_id ON trades(corr_id);
CREATE INDEX IF NOT EXISTS idx_trades_trx_hash ON trades(trx_hash);
CREATE INDEX IF NOT EXISTS idx_timings_corr_id ON timings(corr_id);
CREATE INDEX IF NOT EXISTS idx_errors_corr_id ON errors(corr_id);
CREATE INDEX IF NOT EXISTS idx_outcomes_token_addr ON outcomes(token_addr);
CREATE INDEX IF NOT EXISTS idx_outcomes_corr_id ON outcomes(corr_id);
CREATE INDEX IF NOT EXISTS idx_skipped_token_addr ON skipped(token_addr);
CREATE INDEX IF NOT EXISTS idx_skipped_corr_id ON skipped(corr_id); 