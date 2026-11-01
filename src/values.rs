#[derive(Debug, Clone, Copy)]
pub struct TokenValue {
    pub integer: u64,
    pub float: f64, 
}

impl TokenValue {
    pub const TOKEN_DECIMALS_FACTOR: f64 = 1_000_000.0; // 10^6 since TOKEN_DECIMALS is 6
    pub fn from_integer(integer: u64) -> Self {
        let float = integer as f64 / Self::TOKEN_DECIMALS_FACTOR;
        Self { integer, float }
    }

    pub fn from_float(float: f64) -> Self {
        let integer = (float * Self::TOKEN_DECIMALS_FACTOR).round() as u64;
        Self { integer, float }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct SolValue {
    pub lamports: u64,
    pub sol: f64,
}

impl SolValue {
    pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
    pub const MICRO_LAMPORTS_PER_LAMPORTS: u64 = 1_000_000;

    pub fn from_lamports(lamports: u64) -> Self {
        let sol = lamports as f64 / Self::LAMPORTS_PER_SOL as f64;
        Self { lamports, sol }
    }

    pub fn from_sol(sol: f64) -> Self {
        let lamports = (sol * Self::LAMPORTS_PER_SOL as f64).round() as u64;
        Self { lamports, sol }
    }

    pub fn compute_priority_fee(compute_price_mlamports: u64, compute_units: u64) -> Self {
        let lamports = (compute_price_mlamports as f64 * compute_units as f64 / SolValue::MICRO_LAMPORTS_PER_LAMPORTS as f64) as u64;
        Self::from_lamports(lamports)
    }
}
