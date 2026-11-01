use bytemuck::{Pod, Zeroable};
use anyhow::{anyhow, Result};
use crate::db::Curve;
use crate::values::{SolValue, TokenValue};


#[derive(Clone)]
pub struct CurveDataPoint {
    pub price: SolValue,
    pub price_delta: SolValue,
    pub real_sol_liquidity: SolValue,
    pub real_sol_liquidity_delta: SolValue,
    pub real_token_liquidity: TokenValue,
    pub real_token_liquidity_delta: TokenValue,
    pub virtual_sol_liquidity: SolValue,
    pub virtual_sol_liquidity_delta: SolValue,
    pub virtual_token_liquidity: TokenValue,
    pub virtual_token_liquidity_delta: TokenValue,
}

#[derive(Clone)]
pub struct CurveHistory {
    pub data: Vec<CurveDataPoint>
}

impl CurveHistory {
    pub fn new() -> Self {
        CurveHistory { data: Vec::new() }
    }

    pub fn get_percentage_change(&self, index: usize) -> f64 {
        if self.data.len() < 2 || (index + 1) >= self.data.len() {
            return 0.0;
        }
        let second = self.data.get(self.data.len() - index - 1).unwrap();
        let first = self.data.get(self.data.len() - index - 2).unwrap();
        (second.price.sol - first.price.sol) / first.price.sol
    }

    pub fn get_derivative_volatility(&self, index: usize) -> f64 {
        let second = self.get_percentage_change(index);
        let first = self.get_percentage_change(index + 1);
        (second - first).abs()
    }

    pub fn add_curve(&mut self, curve_state: &CurveState) {
        if self.data.len() == 0 {
            self.data.push(CurveDataPoint {
                price: curve_state.price,
                price_delta: SolValue::from_lamports(0),
                real_sol_liquidity: curve_state.real_sol_reserves,
                real_sol_liquidity_delta: SolValue::from_lamports(0),
                real_token_liquidity: curve_state.real_token_reserves,
                real_token_liquidity_delta: TokenValue::from_integer(0),
                virtual_sol_liquidity: curve_state.virtual_sol_reserves,
                virtual_sol_liquidity_delta: SolValue::from_lamports(0),
                virtual_token_liquidity: curve_state.virtual_token_reserves,
                virtual_token_liquidity_delta: TokenValue::from_integer(0),
            });
            return;
        }
        let last_data_point = self.data.last().unwrap();
        self.data.push(CurveDataPoint {
            price: curve_state.price,
            price_delta: SolValue::from_sol(curve_state.price.sol - last_data_point.price.sol),
            real_sol_liquidity: curve_state.real_sol_reserves,
            real_sol_liquidity_delta: SolValue::from_sol(curve_state.real_sol_reserves.sol - last_data_point.real_sol_liquidity.sol),
            real_token_liquidity: curve_state.real_token_reserves,
            real_token_liquidity_delta: TokenValue::from_float(curve_state.real_token_reserves.float - last_data_point.real_token_liquidity.float),
            virtual_sol_liquidity: curve_state.virtual_sol_reserves,
            virtual_sol_liquidity_delta: SolValue::from_sol(curve_state.virtual_sol_reserves.sol - last_data_point.virtual_sol_liquidity.sol),
            virtual_token_liquidity: curve_state.virtual_token_reserves,
            virtual_token_liquidity_delta: TokenValue::from_float(curve_state.virtual_token_reserves.float - last_data_point.virtual_token_liquidity.float),
        });
    }
}


#[derive(Clone, Debug)]
pub struct CurveState {
    pub virtual_token_reserves: TokenValue,
    pub virtual_sol_reserves: SolValue,
    pub real_token_reserves: TokenValue,
    pub real_sol_reserves: SolValue,
    pub token_total_supply: TokenValue,
    pub complete: u8,
    pub price: SolValue,
}


impl CurveState {
    pub fn new() -> Self {
        Self {
            virtual_token_reserves: TokenValue::from_integer(0),
            virtual_sol_reserves: SolValue::from_lamports(0),
            real_token_reserves: TokenValue::from_integer(0),
            real_sol_reserves: SolValue::from_lamports(0),
            token_total_supply: TokenValue::from_integer(0),
            complete: 0,
            price: SolValue::from_sol(0.0),
        }
    }

     /**
     * Recalculates the state after a purchase of tokens, mutates the state
     * and returns the number of tokens received
     */
    pub fn buy_tokens(
        &mut self,
        buy_amount: SolValue,
    ) -> Result<TokenValue> {
        if self.virtual_token_reserves.integer <= 0
            || self.virtual_sol_reserves.lamports <= 0
        {
            return Err(anyhow!("Invalid reserve state"));
        }

        // Calculate new virtual SOL reserves after purchase
        let new_virtual_sol = SolValue::from_sol(
            self.virtual_sol_reserves.sol + buy_amount.sol
        );

        // Calculate tokens received using constant product formula
        // new_tokens = current_tokens * (1 - old_sol/new_sol)
        let tokens_received = TokenValue::from_float(
            self.virtual_token_reserves.float * 
            (1.0 - self.virtual_sol_reserves.sol / new_virtual_sol.sol)
        );

        // Calculate new virtual token reserves
        let new_virtual_tokens = TokenValue::from_float(
            self.virtual_token_reserves.float - tokens_received.float
        );

        // Calculate new price using new virtual reserves
        let new_price = SolValue::from_sol(
            new_virtual_sol.sol / new_virtual_tokens.float
        );

        // Update state
        self.virtual_sol_reserves = new_virtual_sol;
        self.virtual_token_reserves = new_virtual_tokens;
        self.price = new_price;

        Ok(tokens_received)
    }


    pub fn from_db(price: &Curve) -> Self {
        Self {
            virtual_token_reserves: TokenValue::from_float(price.virtual_token_liquidity),
            virtual_sol_reserves: SolValue::from_sol(price.virtual_sol_liquidity),
            real_token_reserves: TokenValue::from_float(price.real_token_liquidity),
            real_sol_reserves: SolValue::from_sol(price.real_sol_liquidity),
            token_total_supply: TokenValue::from_float(0.0), // TODO: store in db
            complete: 0,
            price: SolValue::from_sol(price.price_sol),
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let bonding_curve_data = CurveData::from_bytes(data)?;
        let virtual_sol_reserves = SolValue::from_lamports(bonding_curve_data.virtual_sol_reserves);
        let virtual_token_reserves = TokenValue::from_integer(bonding_curve_data.virtual_token_reserves);
        
        let price = SolValue::from_sol(
            virtual_sol_reserves.sol / virtual_token_reserves.float,
        );
        Ok(Self {
            virtual_sol_reserves,
            virtual_token_reserves,
            real_sol_reserves: SolValue::from_lamports(bonding_curve_data.real_sol_reserves),
            real_token_reserves: TokenValue::from_integer(bonding_curve_data.real_token_reserves),
            token_total_supply: TokenValue::from_integer(bonding_curve_data.token_total_supply),
            complete: bonding_curve_data.complete,
            price,
        })
    }
}


#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CurveData {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: u8,
}

impl CurveData {
    pub const LEN: usize = 41;

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::LEN {
            return Err(anyhow::anyhow!("Data length too short"));
        }
        
        Ok(unsafe { *(data[8..Self::LEN].as_ptr() as *const Self) })
    }
}

// Safety: This type has a defined C representation and no invalid bit patterns
unsafe impl Pod for CurveData {}
unsafe impl Zeroable for CurveData {} 