use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, PgPool, postgres::PgPoolOptions};
use uuid::Uuid;
use std::time::Duration;
use dotenv::dotenv;
// Models
#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub wallet_addr: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub token_addr: String,
    pub symbol: String,
    pub name: String,
    pub user_addr: String,
    pub bond_curve_addr: String,
    pub assoc_bond_curve_addr: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Curve {
    pub id: i64,
    pub token_addr: String,
    pub timestamp: DateTime<Utc>,
    pub real_sol_liquidity: f64,
    pub real_token_liquidity: f64,
    pub virtual_sol_liquidity: f64,
    pub virtual_token_liquidity: f64,
    pub price_sol: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub trx_hash: String,
    pub token_addr: String,
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub is_simulated: bool,
    pub corr_id: Uuid,
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Timing {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub duration_micros: i64,
    pub action: String,
    pub corr_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub id: i64,
    pub corr_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub context: Option<serde_json::Value>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Strategy {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub buy_amount: f64,
    pub strategy_type: String,
    pub params: Option<serde_json::Value>,
    pub is_active: bool,

    // Transaction Costs
    pub sell_slippage: f64,
    pub buy_slippage: f64,

    pub sell_compute_units: i32,
    pub buy_compute_units: i32,
    
    pub sell_compute_price_mlamports: i64,
    pub buy_compute_price_mlamports: i64,
    
    // Loop Timing Parameters
    pub buy_loop_max_inactive_time_ms: i32,
    pub buy_loop_inactive_interval_ms: i32,
    pub buy_loop_active_interval_ms: i32,
    pub buy_active_volatility_threshold: Option<f64>,

    pub sell_loop_time_ms: i32,
    pub sell_max_time_ms: i32,
    
    // Sell Signal Parameters
    pub sell_signal_percent: f64,
    pub sell_signal_stop_loss_percent: Option<f64>,

    // Buy Signal Parameters
    pub buy_signal_min_percent: Option<f64>,
    pub buy_signal_max_percent: Option<f64>,
    
    pub buy_signal_min_sol_liquidity: Option<f64>,
    pub buy_signal_max_sol_liquidity: Option<f64>,
    
    pub buy_signal_min_volatility: Option<f64>,
    pub buy_signal_max_volatility: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub corr_id: Uuid,
    pub token_addr: String,
    pub buy_trade_hash: String,
    pub sell_trade_hash: String,
    pub duration_seconds: i32,
    pub margin_sol: f64,
    pub roi_percent: f64,
    pub is_simulated: bool,
    pub strategy_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Skipped {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub corr_id: Uuid,
    pub token_addr: String,
    pub duration_seconds: i32,
    pub is_simulated: bool,
    pub strategy_id: i64,
}

#[derive(Debug)]
pub struct StrategyPerformance {
    pub id: i64,
    pub strategy_type: String,
    pub total_runs: i64,
    pub total_outcomes: i64,
    pub total_skipped: i64,
    pub total_invested: f64,
    pub cumulative_margin: f64,
    pub mean_roi_percent: f64,
    pub is_active: bool,
}


pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn new() -> Result<Self> {
        dotenv()?;
        let database_url = std::env::var("DATABASE_URL")?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(&database_url)
            .await?;

        Ok(Self { pool })
    }

    // Wallet operations
    pub async fn insert_wallet(&self, wallet_addr: &str) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO wallets (wallet_addr, timestamp)
            VALUES ($1, NOW())
            ON CONFLICT (wallet_addr) DO NOTHING
            "#,
            wallet_addr,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    // Token operations
    pub async fn insert_token(&self, token: &Token) -> Result<String> {
        // First ensure the wallet exists
        self.insert_wallet(&token.user_addr).await?;

        // Then insert the token
        let record = sqlx::query!(
            r#"
            INSERT INTO tokens (token_addr, symbol, name, user_addr, bond_curve_addr, assoc_bond_curve_addr)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING token_addr
            "#,
            token.token_addr,
            token.symbol,
            token.name,
            token.user_addr,
            token.bond_curve_addr,
            token.assoc_bond_curve_addr,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.token_addr)
    }

    // Price operations
    pub async fn insert_price(&self, price: &Curve) -> Result<i64> {
        let record = sqlx::query!(
            r#"
            INSERT INTO prices (token_addr, real_sol_liquidity, real_token_liquidity, price_sol)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
            price.token_addr,
            price.real_sol_liquidity as f64,
            price.real_token_liquidity as f64,
            price.price_sol as f64,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    pub async fn fetch_all_tokens_prices(&self) -> Result<Vec<(Token, Vec<Curve>)>> {
        // Fetch all tokens
        let tokens = self.fetch_all_tokens().await?;
        println!("Found {} tokens to backtest", tokens.len());
    
        let mut token_data = Vec::new();
        
        // Fetch prices for each token
        for token in tokens {
            println!("Fetching prices for token {}", token.token_addr);
            let prices = self.fetch_prices_by_token_addr(&token.token_addr).await?;
            if !prices.is_empty() {
                token_data.push((token, prices));
            }
        }
    
        Ok(token_data)
    }

    // Trade operations
    pub async fn insert_trade(&self, trade: &Trade) -> Result<String> {
        let record = sqlx::query!(
            r#"
            INSERT INTO trades (trx_hash, token_addr, action, is_simulated, corr_id, price, quantity)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING trx_hash
            "#,
            trade.trx_hash,
            trade.token_addr,
            trade.action,
            trade.is_simulated,
            trade.corr_id,
            trade.price as f64,
            trade.quantity as f64,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.trx_hash)
    }

    // Timing operations
    pub async fn insert_timing(&self, timing: &Timing) -> Result<i64> {
        let record = sqlx::query!(
            r#"
            INSERT INTO timings (duration_micros, action, corr_id)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            timing.duration_micros,
            timing.action,
            timing.corr_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    pub async fn insert_timings_batch(&self, timings: &[Timing]) -> Result<()> {
        if timings.is_empty() {
            return Ok(());
        }

        // Create the base query with the first set of parameters
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO timings (duration_micros, action, corr_id) "
        );

        // Build the VALUES part of the query
        query_builder.push_values(timings, |mut b, timing| {
            b.push_bind(timing.duration_micros)
                .push_bind(&timing.action)
                .push_bind(timing.corr_id);
        });

        // Execute the query
        query_builder
            .build()
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Error operations
    pub async fn insert_error(&self, error: &Error) -> Result<i64> {
        let record = sqlx::query!(
            r#"
            INSERT INTO errors (corr_id, context, message)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            error.corr_id,
            error.context,
            error.message,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    pub async fn get_random_active_strategy(&self) -> Result<Option<Strategy>> {
        let record = sqlx::query!(
            r#"
            SELECT 
                id, timestamp, buy_amount::float8, sell_slippage::float8, buy_slippage::float8, sell_compute_units,
                buy_compute_units, sell_compute_price_mlamports, buy_compute_price_mlamports,
                buy_loop_max_inactive_time_ms, buy_loop_inactive_interval_ms, buy_loop_active_interval_ms,
                sell_loop_time_ms, sell_max_time_ms,
                sell_signal_percent::float8, sell_signal_stop_loss_percent::float8,
                buy_signal_min_percent::float8, buy_active_volatility_threshold::float8,
                buy_signal_min_sol_liquidity::float8, buy_signal_min_volatility::float8,
                buy_signal_max_percent::float8, buy_signal_max_sol_liquidity::float8,
                buy_signal_max_volatility::float8,
                type as "strategy_type!", params, is_active
            FROM strategy 
            WHERE is_active = true 
            ORDER BY RANDOM() 
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(|r| Strategy {
            id: r.id,
            timestamp: r.timestamp,
            buy_amount: r.buy_amount.unwrap_or(0.0),
            sell_slippage: r.sell_slippage.unwrap_or(0.0),
            buy_slippage: r.buy_slippage.unwrap_or(0.0),
            sell_compute_units: r.sell_compute_units,
            buy_compute_units: r.buy_compute_units,
            sell_compute_price_mlamports: r.sell_compute_price_mlamports,
            buy_compute_price_mlamports: r.buy_compute_price_mlamports,
            buy_loop_max_inactive_time_ms: r.buy_loop_max_inactive_time_ms,
            buy_loop_inactive_interval_ms: r.buy_loop_inactive_interval_ms,
            buy_loop_active_interval_ms: r.buy_loop_active_interval_ms,
            buy_active_volatility_threshold: r.buy_active_volatility_threshold,
            sell_loop_time_ms: r.sell_loop_time_ms,
            sell_max_time_ms: r.sell_max_time_ms,
            sell_signal_percent: r.sell_signal_percent.unwrap_or(0.0),
            sell_signal_stop_loss_percent: r.sell_signal_stop_loss_percent,
            buy_signal_min_percent: r.buy_signal_min_percent,
            buy_signal_max_percent: r.buy_signal_max_percent,
            buy_signal_min_sol_liquidity: r.buy_signal_min_sol_liquidity,
            buy_signal_max_sol_liquidity: r.buy_signal_max_sol_liquidity,
            buy_signal_min_volatility: r.buy_signal_min_volatility,
            buy_signal_max_volatility: r.buy_signal_max_volatility,
            strategy_type: r.strategy_type,
            params: r.params,
            is_active: r.is_active,
        }))
    }

    // Outcome operations
    pub async fn insert_outcome(&self, outcome: &Outcome) -> Result<i64> {
        let record = sqlx::query!(
            r#"
            INSERT INTO outcomes (
                timestamp, corr_id, token_addr, buy_trade_hash, sell_trade_hash,
                duration_seconds, margin_sol, roi_percent, is_simulated, strategy_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            outcome.timestamp,
            outcome.corr_id,
            outcome.token_addr,
            outcome.buy_trade_hash,
            outcome.sell_trade_hash,
            outcome.duration_seconds,
            outcome.margin_sol as f64,
            outcome.roi_percent as f64,
            outcome.is_simulated,
            outcome.strategy_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    // Skipped operations
    pub async fn insert_skipped(&self, skipped: &Skipped) -> Result<i64> {
        let record = sqlx::query!(
            r#"
            INSERT INTO skipped (timestamp, corr_id, token_addr, duration_seconds, is_simulated, strategy_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            skipped.timestamp,
            skipped.corr_id,
            skipped.token_addr,
            skipped.duration_seconds,
            skipped.is_simulated,
            skipped.strategy_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    

    pub async fn insert_curves_batch(&self, curves: &[Curve]) -> Result<()> {
        if curves.is_empty() {
            return Ok(());
        }

        // Process in chunks of 1000 records to stay well within Postgres parameter limits
        const CHUNK_SIZE: usize = 1000;
        
        for chunk in curves.chunks(CHUNK_SIZE) {
            let mut query_builder = sqlx::QueryBuilder::new(
                "INSERT INTO prices (token_addr, timestamp, real_sol_liquidity, real_token_liquidity, price_sol, virtual_sol_liquidity, virtual_token_liquidity) "
            );

            query_builder.push_values(chunk, |mut b, price| {
                b.push_bind(&price.token_addr)
                    .push_bind(price.timestamp)
                    .push_bind(price.real_sol_liquidity)
                    .push_bind(price.real_token_liquidity)
                    .push_bind(price.price_sol)
                    .push_bind(price.virtual_sol_liquidity)
                    .push_bind(price.virtual_token_liquidity);
            });

            query_builder.build().execute(&self.pool).await?;
        }

        Ok(())
    }

    pub async fn fetch_prices_by_token_addr(&self, token_addr: &str) -> Result<Vec<Curve>> {
        let records = sqlx::query!(
            r#"
            SELECT 
                id,
                token_addr,
                timestamp,
                real_sol_liquidity::float8 as real_sol_liquidity,
                real_token_liquidity::float8 as real_token_liquidity,
                virtual_sol_liquidity::float8 as virtual_sol_liquidity,
                virtual_token_liquidity::float8 as virtual_token_liquidity,
                price_sol::float8 as price_sol
            FROM prices 
            WHERE token_addr = $1
            ORDER BY timestamp ASC
            "#,
            token_addr
        )
        .fetch_all(&self.pool)
        .await?;

        let prices = records.into_iter().map(|r| Curve {
            id: r.id,
            token_addr: r.token_addr,
            timestamp: r.timestamp,
            real_sol_liquidity: r.real_sol_liquidity.unwrap_or(0.0),
            real_token_liquidity: r.real_token_liquidity.unwrap_or(0.0),
            virtual_sol_liquidity: r.virtual_sol_liquidity.unwrap_or(0.0),
            virtual_token_liquidity: r.virtual_token_liquidity.unwrap_or(0.0),
            price_sol: r.price_sol.unwrap_or(0.0),
        }).collect();

        Ok(prices)
    }

    pub async fn fetch_all_tokens(&self) -> Result<Vec<Token>> {
        let records = sqlx::query!(
            r#"
            SELECT 
                token_addr,
                symbol,
                name,
                user_addr,
                bond_curve_addr,
                assoc_bond_curve_addr,
                timestamp
            FROM tokens
            ORDER BY timestamp DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let tokens = records.into_iter().map(|r| Token {
            token_addr: r.token_addr,
            symbol: r.symbol,
            name: r.name,
            user_addr: r.user_addr,
            bond_curve_addr: r.bond_curve_addr,
            assoc_bond_curve_addr: r.assoc_bond_curve_addr,
            timestamp: r.timestamp,
        }).collect();

        Ok(tokens)
    }

    pub async fn get_strategy_performance(&self) -> Result<Vec<StrategyPerformance>> {
        let query = sqlx::query(
            r#"
            WITH raw_outcomes AS (
                SELECT strategy_id, COUNT(*) as total_count
                FROM outcomes
                GROUP BY strategy_id
            ),
            raw_skipped AS (
                SELECT strategy_id, COUNT(*) as total_count
                FROM skipped
                GROUP BY strategy_id
            ),
            outcome_stats AS (
                SELECT 
                    strategy_id,
                    COUNT(*) as outcome_count,
                    COALESCE(SUM(margin_sol)::FLOAT8, 0.0) as total_margin_sol,
                    COALESCE(AVG(roi_percent)::FLOAT8 * 100.0, 0.0) as mean_roi_percent
                FROM outcomes
                GROUP BY strategy_id
            )
            SELECT 
                s.id,
                s.type as strategy_type,
                COALESCE(ro.total_count, 0) as total_outcomes,
                COALESCE(rs.total_count, 0) as total_skipped,
                (COALESCE(ro.total_count, 0) + COALESCE(rs.total_count, 0)) as total_runs,
                (s.buy_amount::FLOAT8 * COALESCE(ro.total_count, 0)) as total_invested,
                COALESCE(o.total_margin_sol, 0.0) as cumulative_margin,
                COALESCE(o.mean_roi_percent, 0.0) as mean_roi_percent,
                s.is_active
            FROM strategy s
            LEFT JOIN raw_outcomes ro ON s.id = ro.strategy_id
            LEFT JOIN raw_skipped rs ON s.id = rs.strategy_id
            LEFT JOIN outcome_stats o ON s.id = o.strategy_id
            ORDER BY mean_roi_percent DESC;
            "#,
        );

        let rows = query.fetch_all(&self.pool).await?;

        let performances = rows
            .iter()
            .map(|row| StrategyPerformance {
                id: row.get("id"),
                strategy_type: row.get("strategy_type"),
                total_runs: row.get("total_runs"),
                total_outcomes: row.get("total_outcomes"),
                total_skipped: row.get("total_skipped"),
                total_invested: row.get::<f64, _>("total_invested"),
                cumulative_margin: row.get::<f64, _>("cumulative_margin"),
                mean_roi_percent: row.get::<f64, _>("mean_roi_percent"),
                is_active: row.get("is_active"),
            })
            .collect();

        Ok(performances)
    }
} 