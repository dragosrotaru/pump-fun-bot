use anyhow::Result;
use chrono::Utc;
use colored::*;
use hft::db::Database;

fn format_number(num: f64) -> String {
    format!("{:.2}", num)
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = Database::new().await?;
    let strategies = db.get_strategy_performance().await?;
    
    println!("\nStrategy Performance Analysis (as of {})\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
    println!(
        "{:<4} | {:<25} | {:<8} | {:<6} | {:<9} | {:<8} | {:<12} | {:<10} | {:<8}",
        "ID", "Type", "Status", "Runs", "Outcomes", "Skipped", "Invested SOL", "Margin SOL", "Avg ROI%"
    );
    println!("{}", "-".repeat(100));

    for strat in strategies {
        println!(
            "{:<4} | {:<25} | {:<8} | {:<6} | {:<9} | {:<8} | {:>12} | {:>10} | {:>8}",
            strat.id,
            strat.strategy_type,
            if strat.is_active {
                "✓ Active".green()
            } else {
                "✗ Inactive".red()
            },
            strat.total_runs,
            strat.total_outcomes,
            strat.total_skipped,
            format_number(strat.total_invested),
            format_number(strat.cumulative_margin),
            format_number(strat.mean_roi_percent),
        );
    }

    println!("\n* All monetary values are in SOL");
    println!("* Average ROI % is calculated only from completed trades (outcomes)");
    println!("* Invested SOL = buy_amount * number of outcomes");
    println!("* Margin SOL = cumulative profit/loss from all trades");

    Ok(())
}