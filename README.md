# pump-fun-bot

An experimental high-frequency trading bot for [pump.fun](https://pump.fun) tokens on Solana, written in Rust.

> **Archived / demonstration only.** This project is no longer developed. It is shared to show the approach, not as something to run with real money. Trading freshly launched meme tokens is extremely risky, and most of them go to zero.

## What it does

The bot is split into small binaries that talk over ZeroMQ and log everything to PostgreSQL:

| Binary | Purpose |
| --- | --- |
| `detector` | Subscribes to Solana over WebSocket, watches the pump.fun program for new token launches and publishes them on a ZeroMQ socket. |
| `trader` | Subscribes to new tokens, follows each token's bonding curve and runs the active strategies from the database (buy signal, sell signal, stop loss, minimum liquidity, max hold time). It buys and sells through pump.fun's on-chain program. |
| `analyze` | Prints per-strategy performance: runs, outcomes, SOL invested, margin and average ROI. |
| `backtest` | Replays recorded price data against strategy parameters. |

Other pieces:

- `src/pump_fun_client.rs`: builds pump.fun buy/sell instructions and works out costs and values from the bonding curve, with failover across several RPC endpoints.
- `migrations/`: the PostgreSQL schema (strategies, tokens, prices, trades, timings, outcomes, skips).
- `learning-examples/`: the Python scripts this project started from, taken from [chainstacklabs/pump-fun-bot](https://github.com/chainstacklabs/pump-fun-bot). The git history before this project's commits is theirs.
- `utils/`: small Node helpers (a raw WebSocket listener, and a base58 → Rust byte-array converter for pubkeys).

## Running it (if you really want to)

`DRY_RUN` in `src/config.rs` is `true` by default. In that mode trades are only simulated and logged, and a throwaway keypair is generated at startup.

1. Start Postgres: `docker compose up -d`
2. Copy `.env.example` to `.env` and set `DATABASE_URL`, `RPC_ENDPOINTS` and `WSS_ENDPOINTS` (comma-separated; the public Solana endpoints are rate-limited and too slow for this).
3. Apply migrations: `cargo install sqlx-cli && sqlx migrate run` (see [DATABASE.md](DATABASE.md)).
4. Build: `cargo build --release`. sqlx checks queries at compile time, so the database must be running and migrated.
5. Run `detector` and `trader` in separate terminals, then `analyze` to see results.

Live trading needs `DRY_RUN = false` and a Solana keypair at `key.json` (a JSON array of 64 bytes, as written by `solana-keygen`). That file is gitignored. Never commit it.

## Development notes

Notes from when the project was active. Most of the TODOs were never done.

### Best parameters found

- Buy amount: 0.0405 SOL (profit: 0.0095 SOL)
- Min SOL liquidity: 0.40 SOL
- Buy signal: 1.50%
- Min volatility: 0.00
- Sell signal: 6.10%
- Stop loss: 5.00%

### Strategy ideas

- Trade on long-lived tokens
- Liquidity modifier for max hold time
- Dynamic transaction fees and loop timing
- Atomic buy/sell with Jito bundles
- Buy immediately at launch
- Keyword and copycat tokens and their profitability
- Creator blacklist/whitelist based on past profitability

### Performance and maintenance

- Track performance per RPC client
- Dual WebSocket connections with deduplication and failover
- Pre-sign buy/sell transactions; create token accounts early
- Move to tokio workers for scaling
- Send transactions directly to validators, co-locate with RPC nodes, or run a validator
- Exponential backoff, and turn off strategies that become unprofitable
- Track CAD prices for tax

## License

No license is granted for the original code in this repository. The `learning-examples/` code and earlier history come from [chainstacklabs/pump-fun-bot](https://github.com/chainstacklabs/pump-fun-bot) and are under that project's terms.
