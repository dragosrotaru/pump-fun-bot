use anyhow::{anyhow, Result};
use hft::trade::{buy_signal, calculate_current_value, calculate_profit, calculate_purchase_cost, calculate_roi, calculate_sell_costs};
use hft::db::{Database, Curve, Strategy};
use hft::curve_state::{CurveState, CurveHistory};
use hft::values::{SolValue, TokenValue};

#[derive(Debug)]
struct BacktestResult {
    token_addr: String,
    token_name: String,
    total_trades: usize,
    profitable_trades: usize,
    total_profit: f64,
    max_drawdown: f64,
    win_rate: f64,
    avg_roi: f64,
}

impl BacktestResult {
    pub fn new() -> Self {
        BacktestResult {
            token_addr: String::new(),
            token_name: String::new(),
            total_trades: 0,
            profitable_trades: 0,
            total_profit: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
            avg_roi: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct AggregatedStats {
    total_tokens: usize,
    total_trades: usize,
    total_profitable_trades: usize,
    total_profit: f64,
    avg_win_rate: f64,
    avg_roi: f64,
    max_drawdown: f64,
}

impl AggregatedStats {
    fn from_results(results: &[BacktestResult]) -> Self {
        let total_trades: usize = results.iter().map(|r| r.total_trades).sum();
        let total_profitable_trades: usize = results.iter().map(|r| r.profitable_trades).sum();
        let total_profit: f64 = results.iter().map(|r| r.total_profit).sum();
        
        let avg_win_rate = if total_trades > 0 {
            total_profitable_trades as f64 / total_trades as f64
        } else {
            0.0
        };
    
        let avg_roi: f64 = if total_trades > 0 {
            results.iter().map(|r| r.avg_roi * r.total_trades as f64).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };
    
        let max_drawdown = results.iter()
            .map(|r| r.max_drawdown)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
    
        AggregatedStats {
            total_tokens: results.len(),
            total_trades,
            total_profitable_trades,
            total_profit,
            avg_win_rate,
            avg_roi,
            max_drawdown,
        }
    }

    fn print(&self) {
        println!("\n=== Aggregated Results Across All Tokens ===");
        println!("Total Number of Tokens Tested: {}", self.total_tokens);
        println!("Total Trades Across All Tokens: {}", self.total_trades);
        println!("Total Profitable Trades: {}", self.total_profitable_trades);
        println!("Total Profit: {:.4} SOL", self.total_profit);
        println!("Average Win Rate: {:.2}%", self.avg_win_rate * 100.0);
        println!("Average ROI per Trade: {:.2}%", self.avg_roi * 100.0);
        println!("Worst Max Drawdown: {:.2}%", self.max_drawdown * 100.0);
    }
    
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = Database::new().await?;

    // Get active strategy as base strategy
    let mut base_strategy = match db.get_random_active_strategy().await? {
        Some(s) => s,
        None => {
            return Err(anyhow!("No active strategies found"));
        }
    };

    println!("Testing base strategy: {}", base_strategy.id);

    // Fetch all necessary data
    let token_data = db.fetch_all_tokens_prices().await?;
    
    let mut best_stats: Option<AggregatedStats> = None;
    let mut best_strategy = base_strategy.clone();

    println!("\nTesting parameter combinations...\n");

    // Parameter ranges
    // Stop loss - test from 0% to 10%
    let mut stop_loss = 0.05;
    while stop_loss <= 0.10 {
        base_strategy.sell_signal_stop_loss_percent = Some(stop_loss);

        // Sell signal percent (profit target) - test from 1% to 20%
        let mut sell_signal = 0.001;
        while sell_signal <= 0.5 {
            base_strategy.sell_signal_percent = sell_signal;

            // Min volatility - test from 0.0005 to 0.01
            let mut buy_signal_min_volatility = 0.0000;
            while buy_signal_min_volatility <= 0.1 {
                base_strategy.buy_signal_min_volatility = Some(buy_signal_min_volatility);

                // Buy signal percent - test from 0% to 5%
                let mut buy_signal_min_percent = 0.0000;
                while buy_signal_min_percent <= 0.05 {
                    base_strategy.buy_signal_min_percent = Some(buy_signal_min_percent);

                    // Min SOL liquidity - test from 0.0005 to 10 SOL
                    let mut buy_signal_min_sol_liquidity = 0.0000;
                    while buy_signal_min_sol_liquidity <= 0.8 {
                        base_strategy.buy_signal_min_sol_liquidity = Some(buy_signal_min_sol_liquidity);

                        // Buy amount
                        let mut buy_amount = 0.0005;
                        while buy_amount <= 0.05 {
                            base_strategy.buy_amount = buy_amount;

                            println!("\nTesting parameters:");
                            println!("Buy amount: {} SOL", buy_amount);
                            println!("Min SOL liquidity: {} SOL", buy_signal_min_sol_liquidity);
                            println!("Buy signal: {}%", buy_signal_min_percent * 100.0);
                            println!("Min volatility: {}", buy_signal_min_volatility);
                            println!("Sell signal: {}%", sell_signal * 100.0);
                            println!("Stop loss: {}%", stop_loss * 100.0);

                            // Print best parameters so far if they exist
                            if let Some(ref best) = best_stats {
                                println!("\nBest parameters so far:");
                                println!("Buy amount: {:.4} SOL (profit: {:.4} SOL)", best_strategy.buy_amount, best.total_profit);
                                println!("Min SOL liquidity: {:.2} SOL", best_strategy.buy_signal_min_sol_liquidity.unwrap());
                                println!("Buy signal: {:.2}%", best_strategy.buy_signal_min_percent.unwrap() * 100.0);
                                println!("Min volatility: {:.2}", best_strategy.buy_signal_min_volatility.unwrap());
                                println!("Sell signal: {:.2}%", best_strategy.sell_signal_percent * 100.0);
                                println!("Stop loss: {:.2}%", best_strategy.sell_signal_stop_loss_percent.unwrap() * 100.0);
                            }

                            let mut all_results = Vec::new();

                            // Run backtest for each token with current parameters
                            for (token, prices) in token_data.iter() {
                                match run_backtest(&base_strategy, prices) {
                                    Ok(mut result) => {
                                        result.token_addr = token.token_addr.clone();
                                        result.token_name = token.name.clone();
                                        all_results.push(result);
                                    }
                                    Err(e) => {
                                        println!("Error backtesting token {}: {}", token.token_addr, e);
                                    }
                                }
                            }

                            // Calculate and store stats for this parameter combination
                            if !all_results.is_empty() {
                                let stats = AggregatedStats::from_results(&all_results);
                                
                                // Update best stats if this run has better total profit
                                if best_stats.is_none() || stats.total_profit > best_stats.as_ref().unwrap().total_profit {
                                    best_stats = Some(stats.clone());
                                    best_strategy.buy_amount = buy_amount;
                                    best_strategy.sell_signal_percent = sell_signal;
                                    best_strategy.sell_signal_stop_loss_percent = Some(stop_loss);
                                    best_strategy.buy_signal_min_percent = Some(buy_signal_min_percent);
                                    best_strategy.buy_signal_min_sol_liquidity = Some(buy_signal_min_sol_liquidity);
                                    best_strategy.buy_signal_min_volatility = Some(buy_signal_min_volatility);
                                }

                                stats.print();
                            }
                            buy_amount += 0.01;
                        }
                        buy_signal_min_sol_liquidity += 0.01;
                    }
                    buy_signal_min_percent += 0.005;
                }
                buy_signal_min_volatility += 0.005;
            }
            sell_signal += 0.01;
        }
        stop_loss += 0.01;
    }

    // Print best results
    if let Some(ref best_stats) = best_stats {
        println!("\n=== Best Results ===");
        println!("Best Parameters:");
        println!("Buy amount: {:.4} SOL", best_strategy.buy_amount);
        println!("Sell signal: {:.2}%", best_strategy.sell_signal_percent * 100.0);
        println!("Stop loss: {:?}%", best_strategy.sell_signal_stop_loss_percent.map(|x| x * 100.0));
        println!("Buy signal: {:?}%", best_strategy.buy_signal_min_percent.map(|x| x * 100.0));
        println!("Max buy signal: {:?}%", best_strategy.buy_signal_max_percent.map(|x| x * 100.0));
        println!("Min SOL liquidity: {:?} SOL", best_strategy.buy_signal_min_sol_liquidity);
        println!("Max SOL liquidity: {:?} SOL", best_strategy.buy_signal_max_sol_liquidity);
        println!("Min volatility: {:?}", best_strategy.buy_signal_min_volatility);
        println!("Max volatility: {:?}", best_strategy.buy_signal_max_volatility);
        println!("\nStats:");
        best_stats.print();
    }

    Ok(())
}

fn run_backtest(strategy: &Strategy, curves: &[Curve]) -> Result<BacktestResult> {
    let mut result = BacktestResult::new();
    let mut curve_history = CurveHistory::new();

    let mut in_position = false;
    let mut entry_price = 0.0;
    let mut current_drawdown = 0.0;
    let mut total_roi = 0.0;

    let buy_amount = SolValue::from_sol(strategy.buy_amount);
    let purchase_cost = calculate_purchase_cost(&strategy, buy_amount);

    for curve in curves {
        let mut curve_state = CurveState::from_db(curve);
        curve_history.add_curve(&curve_state);


        if !in_position {
            // Check buy signals
            let buy_signal = buy_signal(strategy.clone(), curve_state.clone(), curve_history.clone());

            if buy_signal {
                entry_price = curve.price_sol;
                in_position = true;
            }
        } else {
            // this purchase will reflect in the curve state (price will change) but it is
            // not realistic since the market would react to the purchase
            curve_state.buy_tokens(buy_amount)?;
            let current_value = calculate_current_value(curve_state.price, TokenValue::from_float(buy_amount.sol / entry_price));
            let selling_cost = calculate_sell_costs(current_value, strategy);
            let profit = calculate_profit(current_value, purchase_cost, selling_cost);
            let roi = calculate_roi(profit, purchase_cost);
            
            // Update max drawdown
            if roi < current_drawdown {
                current_drawdown = roi;
                if current_drawdown < result.max_drawdown {
                    result.max_drawdown = current_drawdown;
                }
            }

            // Check sell signals
            let stop_loss_triggered = strategy.sell_signal_stop_loss_percent.is_some() && 
                roi <= -strategy.sell_signal_stop_loss_percent.unwrap();
            let in_profit_triggered = roi >= strategy.sell_signal_percent;
            
            if in_profit_triggered || stop_loss_triggered {
                // Close position
                result.total_trades += 1;
                if roi > 0.0 {
                    result.profitable_trades += 1;
                }
                result.total_profit += profit.sol;
                total_roi += roi;
                
                in_position = false;
                current_drawdown = 0.0;
                break;
            }
        }
    }

    // Calculate final metrics
    if result.total_trades > 0 {
        result.win_rate = result.profitable_trades as f64 / result.total_trades as f64;
        result.avg_roi = total_roi / result.total_trades as f64;
    }

    Ok(result)
}