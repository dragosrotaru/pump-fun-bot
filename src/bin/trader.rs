use anyhow::anyhow;
use anyhow::Result;
use chrono::Utc;
use colored::*;
use hft::config::DRY_RUN;
use hft::db::Database;
use hft::trade::{
    buy_signal, calculate_current_value, calculate_profit, calculate_purchase_cost,
    calculate_sell_costs,
};
use hft::timer::TimerSeq;
use hft::mq::TokenSubscriber;
use hft::logger::Logger;
use hft::pump_fun_client::PumpFunClient;
use hft::values::SolValue;
use hft::websocket::CreateTokenArgs;
use hft::curve_state::CurveHistory;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("WARNING: DRY_RUN={}", {
        if DRY_RUN {
            "true".green().bold()
        } else {
            "false".red().bold()
        }
    });

    let db = Database::new().await?;
    let logger = Logger::new(&db).await?;
    let mut subscriber = TokenSubscriber::new(Uuid::new_v4().to_string())?;
    subscriber.set_idle(true)?;

    let mut consecutive_errors = 0;
    let mut base_wait_time = 1; // Base wait time in seconds
    let max_wait_time = 300; // Maximum wait time of 5 minutes

    loop {
        let corr_id = Uuid::new_v4();
        let token_msg = subscriber.receive()?;
        subscriber.set_idle(false)?;
        let token_data = token_msg.token;
        match run_trading_cycle(&logger, &db, &token_data, corr_id).await {
            Ok(_) => {
                subscriber.set_idle(true)?;

                // Reset error counter and wait time on success
                consecutive_errors = 0;
                base_wait_time = 1;

                println!("Trading cycle completed successfully. Waiting for next cycle...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                subscriber.set_idle(true)?;

                consecutive_errors += 1;
                let wait_time =
                    (base_wait_time * 2_u64.pow(consecutive_errors.min(8) - 1)).min(max_wait_time);

                // Log the error with context
                let context = serde_json::json!({
                    "consecutive_errors": consecutive_errors,
                    "next_retry_wait_time": wait_time,
                    "error_type": format!("{:?}", e)
                });

                if let Err(log_err) = 
                    logger
                    .log_error(&e.to_string(), Some(context), corr_id)
                    .await
                {
                    eprintln!("Failed to log error: {}", log_err);
                }

                eprintln!("Error during trading cycle: {}", e);
                eprintln!("Consecutive errors: {}", consecutive_errors);
                eprintln!("Waiting {} seconds before retry...", wait_time);

                tokio::time::sleep(Duration::from_secs(wait_time)).await;
            }
        }
    }
}

async fn run_trading_cycle<'a>(
    logger: &'a Logger<'a>,
    db: &'a Database,
    token_data: &CreateTokenArgs,
    corr_id: Uuid,
) -> Result<()> {
    let mut bot = PumpFunClient::new(logger).await?;
    let mut timer = TimerSeq::start("run_trading_cycle");
    let entry_time = Utc::now();
    let mut curve_history = CurveHistory::new();

    // Get a random active strategy
    timer.next("get_random_active_strategy");
    let strategy = match db.get_random_active_strategy().await? {
        Some(s) => s,
        None => {
            println!("{}", "No active strategies found".red().bold());
            return Ok(());
        }
    };
    let buy_amount = SolValue::from_sol(strategy.buy_amount);
    let volatility_trigger = strategy.buy_active_volatility_threshold.unwrap_or(0.005); // todo this should not be nullable

    println!(
        "Selected strategy: {} (ID: {})",
        strategy.strategy_type.green().bold(),
        strategy.id
    );

    println!("Token:\n{}", serde_json::to_string_pretty(&token_data)?);

    // Get Token details
    let mint: Pubkey = Pubkey::from_str(&token_data.mint)?;
    let bonding_curve = Pubkey::from_str(&token_data.bonding_curve)?;
    let associated_bonding_curve = Pubkey::from_str(&token_data.associated_bonding_curve)?;
    let associated_token_account =
        spl_associated_token_account::get_associated_token_address(&bot.payer.pubkey(), &mint);

    println!("Buy Loop");

    // price
    let mut curve_state = bot.fetch_curve_state(&bonding_curve, &mut curve_history, &mut timer)?;

    // Tracking variables
    let mut iterations = 0;
    let mut cooldown_counter = 0;
    let volatility_cooldown = 5;

    timer.next("enter_buy_loop");
    loop {
        let price_increase_percentage = curve_history.get_percentage_change(0);
        let derivative_volatility = curve_history.get_derivative_volatility(0);

        let buy_signal = buy_signal(strategy.clone(), curve_state.clone(), curve_history.clone());

        timer.next("check_buy_signal");
        if buy_signal {
            println!("{}", "buy signal triggered".green().bold());
            break;
        }

        if derivative_volatility > volatility_trigger {
            cooldown_counter = volatility_cooldown;
        } else if cooldown_counter > 0 {
            cooldown_counter -= 1;
        }

        let is_active = cooldown_counter > 0;

        println!(
            "Movement: {:.2}% • Volatility: {:.2}%",
            price_increase_percentage * 100.0,
            derivative_volatility * 100.0
        );

        if !is_active {
            iterations += 1;
            println!("Iterations: {}", iterations);
        } else {
            iterations = 0;
            println!("{}", "Volatility".yellow().bold());
        }

        if (iterations * strategy.buy_loop_inactive_interval_ms)
            > strategy.buy_loop_max_inactive_time_ms
        {
            println!("{}", "Skipping Token".yellow().bold());
            timer.next("log_skip_token");
            bot.logger
                .log_skipped(corr_id, &token_data, entry_time, strategy.id)
                .await?;
            bot.logger
                .log_curve_history(&token_data.mint, &curve_history.data)
                .await?;
            timer.end();
            bot.logger
                .log_timing(corr_id, "run_trading_cycle", &mut timer)
                .await?;

            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(if is_active {
            strategy.buy_loop_active_interval_ms as u64
        } else {
            strategy.buy_loop_inactive_interval_ms as u64
        }))
        .await;

        curve_state = bot.fetch_curve_state(&bonding_curve, &mut curve_history, &mut timer)?;
    }
    timer.next("execute_buy");

    // Execute buy
    let (tokens_received, cost_with_slippage) = bot
        .buy_token(
            &mint,
            &bonding_curve,
            &associated_bonding_curve,
            &associated_token_account,
            buy_amount,
            &mut curve_state,
            corr_id,
            &strategy,
        )
        .await?;
    let purchase_cost = calculate_purchase_cost(&strategy, cost_with_slippage);
    // Get token balance
    let mut token_balance = tokens_received;
    if !DRY_RUN {
        timer.next("get_token_balance");
        token_balance = bot.get_token_balance(&associated_token_account).await?
    }
    if token_balance.integer == 0 {
        return Err(anyhow!("No tokens to sell"));
    }

    println!("{}", "Sell Loop".yellow().bold());
    let mut iterations = 0;
    timer.next("enter_sell_loop");
    loop {
        let current_value = calculate_current_value(curve_state.price, token_balance);
        let selling_cost = calculate_sell_costs(current_value, &strategy);
        let profit = calculate_profit(current_value, purchase_cost, selling_cost);
        let roi = profit.sol / purchase_cost.sol;

        println!("Profit: {:.2}%", roi * 100.0);

        let in_profit_triggered = roi >= strategy.sell_signal_percent;
        let stop_loss_triggered = strategy.sell_signal_stop_loss_percent.is_some()
            && roi <= -strategy.sell_signal_stop_loss_percent.unwrap();
        let max_time_triggered =
            (iterations * strategy.sell_loop_time_ms) > strategy.sell_max_time_ms;

        timer.next("check_sell_conditions");
        if in_profit_triggered || stop_loss_triggered || max_time_triggered {
            let sell_tx_hash = bot
                .sell_token(
                    &mint,
                    &bonding_curve,
                    &associated_bonding_curve,
                    &associated_token_account,
                    corr_id,
                    token_balance,
                    &curve_state,
                    &strategy,
                )
                .await?;

            if in_profit_triggered {
                println!("\x07"); // ring the bell
                println!(
                    "{} ({} SOL, {}%)",
                    "Profit".green().bold(),
                    format!("{:.4}", profit.sol).green(),
                    format!("{:.2}", roi * 100.0).green()
                );
            } else if stop_loss_triggered {
                println!(
                    "{} ({} SOL, {}%)",
                    "Stop Loss Triggered".yellow().bold(),
                    format!("{:.4}", profit.sol).red(),
                    format!("{:.2}", roi * 100.0).red()
                );
            } else {
                println!(
                    "{} ({} SOL, {}%)",
                    "Loss".red().bold(),
                    format!("{:.4}", profit.sol).red(),
                    format!("{:.2}", roi * 100.0).red()
                );
            }

            bot.logger
                .log_outcome(
                    corr_id,
                    &token_data,
                    &sell_tx_hash,
                    &sell_tx_hash,
                    entry_time,
                    profit,
                    roi,
                    strategy.id,
                )
                .await?;
            bot.logger
                .log_curve_history(&token_data.mint, &curve_history.data)
                .await?;
            timer.end();
            bot.logger
                .log_timing(corr_id, "run_trading_cycle", &mut timer)
                .await?;
            break;
        }

        iterations += 1;
        tokio::time::sleep(Duration::from_millis(strategy.sell_loop_time_ms as u64)).await;

        // the current price of the token in lamports
        curve_state = bot.fetch_curve_state(&bonding_curve, &mut curve_history, &mut timer)?;
        if DRY_RUN {
            curve_state.buy_tokens(buy_amount)?;
        }
    }

    Ok(())
}

