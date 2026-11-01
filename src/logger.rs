use anyhow::Result;
use chrono::{Utc, DateTime};
use solana_sdk::pubkey::Pubkey;
use uuid::Uuid;
use crate::websocket::CreateTokenArgs;
use crate::db::{Database, Curve};
use crate::values::{SolValue, TokenValue};
use crate::timer::TimerSeq;
use crate::config::DRY_RUN;

pub struct Logger<'a> {
    db: &'a Database,
}

impl<'a> Logger<'a> {
    pub async fn new(db: &'a Database) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn log_trade(
        &self,
        corr_id: Uuid,
        action: &str,
        mint: &Pubkey,
        price: SolValue,
        quantity: TokenValue,
        tx_hash: &str,
    ) -> Result<String> {
        let trade = crate::db::Trade {
            trx_hash: tx_hash.to_string(),
            token_addr: mint.to_string(),
            timestamp: Utc::now(),
            action: action.to_string(),
            is_simulated: DRY_RUN,
            corr_id,
            price: price.sol,
            quantity: quantity.float,
        };

        self.db.insert_trade(&trade).await
    }


    pub async fn log_curve_history(&self, token_addr: &str, data_points: &[crate::curve_state::CurveDataPoint]) -> Result<()> {
        let curves: Vec<Curve> = data_points
            .iter()
            .map(|dp| Curve {
                id: 0, // Will be set by the database
                token_addr: token_addr.to_string(),
                timestamp: Utc::now(),
                real_sol_liquidity: dp.real_sol_liquidity.sol,
                real_token_liquidity: dp.real_token_liquidity.float,
                virtual_sol_liquidity: dp.virtual_sol_liquidity.sol,
                virtual_token_liquidity: dp.virtual_token_liquidity.float,
                price_sol: dp.price.sol,
            })
            .collect();

        self.db.insert_curves_batch(&curves).await
    }

    pub async fn log_timing(
        &self,
        corr_id: Uuid,
        action: &str,
        timer: &mut TimerSeq,
    ) -> Result<()> {
        timer.end();
        let timings: Vec<crate::db::Timing> = timer.timers
            .iter()
            .map(|timer| crate::db::Timing {
                id: 0, // Will be set by database
                timestamp: timer.wall_time.unwrap(),
                duration_micros: timer.duration(),
                action: format!("{}_{}", action, timer.action),
                corr_id,
            })
            .collect();

        self.db.insert_timings_batch(&timings).await
    }

    pub async fn log_error(
        &self,
        error: &str,
        context: Option<serde_json::Value>,
        corr_id: Uuid,
    ) -> Result<i64> {
        let error_entry = crate::db::Error {
            id: 0, // Will be set by database
            corr_id,
            timestamp: Utc::now(),
            context,
            message: error.to_string(),
        };

        self.db.insert_error(&error_entry).await
    }

    pub async fn log_token(
        &self,
        token_data: CreateTokenArgs,
    ) -> Result<String> {
        let token = crate::db::Token {
            token_addr: token_data.mint.clone(),
            symbol: token_data.symbol.clone(),
            name: token_data.name.clone(),
            user_addr: token_data.user.clone(),
            bond_curve_addr: token_data.bonding_curve.clone(),
            assoc_bond_curve_addr: token_data.associated_bonding_curve.clone(),
            timestamp: Utc::now(),
        };
        self.db.insert_token(&token).await
    }

    pub async fn log_outcome(
        &self,
        corr_id: Uuid,
        token_data: &CreateTokenArgs,
        buy_tx_hash: &str,
        sell_tx_hash: &str,
        entry_time: DateTime<Utc>,
        profit: SolValue,
        roi: f64,
        strategy_id: i64,
    ) -> Result<i64> {
        let duration_seconds = (Utc::now() - entry_time).num_seconds() as i32;
        let outcome = crate::db::Outcome {
            id: 0, // Will be set by database
            timestamp: Utc::now(),
            corr_id,
            token_addr: token_data.mint.clone(),
            buy_trade_hash: buy_tx_hash.to_string(),
            sell_trade_hash: sell_tx_hash.to_string(),
            duration_seconds,
            margin_sol: profit.sol,
            roi_percent: roi,
            is_simulated: DRY_RUN,
            strategy_id,
        };

        self.db.insert_outcome(&outcome).await
    }

    pub async fn log_skipped(
        &self,
        corr_id: Uuid,
        token_data: &CreateTokenArgs,
        entry_time: DateTime<Utc>,
        strategy_id: i64,
    ) -> Result<i64> {
        let duration_seconds = (Utc::now() - entry_time).num_seconds() as i32;
        let skipped = crate::db::Skipped {
            id: 0, // Will be set by database
            timestamp: Utc::now(),
            corr_id,
            token_addr: token_data.mint.clone(),
            duration_seconds,
            is_simulated: DRY_RUN,
            strategy_id,
        };

        self.db.insert_skipped(&skipped).await
    }
} 