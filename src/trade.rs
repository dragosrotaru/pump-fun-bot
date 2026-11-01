use crate::curve_state::{CurveState, CurveHistory};
use crate::values::{SolValue, TokenValue};
use crate::db::Strategy;
use crate::config::{BASE_TRX_COST, PUMP_FEE_PERCENTAGE};
/**
 * Returns true if all the strategy conditions are met to buy
 */
pub fn buy_signal(strategy: Strategy, curve_state: CurveState, curve_history: CurveHistory) -> bool {
    let price_increase = curve_history.get_percentage_change(0);
    let derivative_volatility = curve_history.get_derivative_volatility(0);

    let buy_signal = 
                (strategy.buy_signal_min_percent.is_some() && price_increase >= strategy.buy_signal_min_percent.unwrap()) &&
                (strategy.buy_signal_min_sol_liquidity.is_some() && curve_state.real_sol_reserves.sol >= strategy.buy_signal_min_sol_liquidity.unwrap()) &&
                (strategy.buy_signal_min_volatility.is_some() && derivative_volatility >= strategy.buy_signal_min_volatility.unwrap()) &&
                (strategy.buy_signal_max_percent.is_some() && price_increase <= strategy.buy_signal_max_percent.unwrap()) &&
                (strategy.buy_signal_max_sol_liquidity.is_some() && curve_state.real_sol_reserves.sol <= strategy.buy_signal_max_sol_liquidity.unwrap()) &&
                (strategy.buy_signal_max_volatility.is_some() && derivative_volatility <= strategy.buy_signal_max_volatility.unwrap());

    return buy_signal;
}

/**
 * Calculates the current value of the tokens held in the wallet
 */
pub fn calculate_current_value(token_price: SolValue, token_balance: TokenValue) -> SolValue {
    let current_value = SolValue::from_sol(token_price.sol * token_balance.float);
    return current_value;
}


/**
 * The selling costs include the pump.fun % fee, the sell slippage and the Transaction cost
 */
pub fn calculate_sell_costs(current_value: SolValue, strategy: &Strategy) -> SolValue {
    let priority_fee = SolValue::compute_priority_fee(strategy.sell_compute_price_mlamports as u64, strategy.sell_compute_units as u64);
    
    let pump_fee = SolValue::from_sol(current_value.sol * PUMP_FEE_PERCENTAGE);
    let sell_slippage = SolValue::from_sol(current_value.sol * strategy.sell_slippage);
    
    SolValue::from_sol(
        pump_fee.sol
            + sell_slippage.sol
            + priority_fee.sol
            + BASE_TRX_COST.sol,
    )
}

pub fn calculate_priority_fee(strategy: &Strategy) -> SolValue {
    let priority_fee = SolValue::compute_priority_fee(
        strategy.sell_compute_price_mlamports as u64,
        strategy.sell_compute_units as u64,
    );
    return priority_fee;
}

/**
 * Calculates the purchase cost including the pump.fun % fee, the buy slippage and the Transaction cost
 */
pub fn calculate_purchase_cost(strategy: &Strategy, cost_with_slippage: SolValue) -> SolValue {
    let priority_fee = calculate_priority_fee(strategy);
    let purchase_cost = SolValue::from_sol(
        cost_with_slippage.sol * (1.0 + PUMP_FEE_PERCENTAGE) + BASE_TRX_COST.sol + priority_fee.sol,
    );
    return purchase_cost;
}

/**
 * Calculates the profit from the current value, purchase cost and selling cost
 */
pub fn calculate_profit(current_value: SolValue, purchase_cost: SolValue, selling_cost: SolValue) -> SolValue {
    let profit = SolValue::from_sol(current_value.sol - purchase_cost.sol - selling_cost.sol);
    return profit;
}

pub fn calculate_roi(profit: SolValue, purchase_cost: SolValue) -> f64 {
    let roi = profit.sol / purchase_cost.sol;
    return roi;
}   