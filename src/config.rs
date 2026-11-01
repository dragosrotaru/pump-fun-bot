use solana_sdk::pubkey::Pubkey;
use crate::values::SolValue;
use std::fs;
use anyhow::Result;

pub static DRY_RUN: bool = true;

// pump.fun fee percentage
pub static PUMP_FEE_PERCENTAGE: f64 = 0.01;

pub static BASE_TRX_COST: SolValue = SolValue {
    sol: 0.000005,
    lamports: 5000
};

// Variables
// Comma-separated lists, read from the environment (or .env).
// Falls back to Solana's public mainnet endpoints.
fn endpoints_from_env(var: &str, default: &str) -> Vec<String> {
    dotenv::dotenv().ok();
    std::env::var(var)
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

lazy_static::lazy_static! {
    pub static ref RPC_ENDPOINTS: Vec<String> =
        endpoints_from_env("RPC_ENDPOINTS", "https://api.mainnet-beta.solana.com");
    pub static ref WSS_ENDPOINTS: Vec<String> =
        endpoints_from_env("WSS_ENDPOINTS", "wss://api.mainnet-beta.solana.com");
}

// Pump.Fun addresses
// for listening to raydium migration (not used)
// "39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg";

// "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMP_PROGRAM_PUBKEY: Pubkey = Pubkey::new_from_array([
    1, 86, 224, 246, 147, 102, 90, 207, 68, 219, 21, 104, 191, 23, 91, 170, 81, 137, 203, 151, 245, 210, 255, 59, 101, 93, 43, 182, 253, 109, 24, 176
]);
// "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf";
pub const PUMP_GLOBAL_PUBKEY: Pubkey = Pubkey::new_from_array([
    58, 134, 94, 105, 238, 15, 84, 128, 202, 188, 246, 99, 87, 228, 220, 47, 24, 213, 141, 69, 193, 234, 116, 137, 251, 55, 35, 217, 121, 60, 114, 166
]);
// "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1";
pub const PUMP_EVENT_AUTHORITY_PUBKEY: Pubkey = Pubkey::new_from_array([
    172, 241, 54, 235, 1, 252, 28, 78, 136, 61, 35, 200, 181, 132, 74, 181, 154, 55, 246, 106, 221, 87, 197, 233, 172, 59, 83, 224, 89, 211, 92, 100
]);
// "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM";
pub const PUMP_FEE_PUBKEY: Pubkey = Pubkey::new_from_array([
    173, 17, 230, 164, 252, 41, 68, 164, 250, 130, 81, 190, 248, 21, 66, 110, 27, 251, 40, 198, 182, 100, 102, 119, 96, 124, 106, 217, 245, 102, 166, 70
]);

// Discriminators
// bonding curve 6966180631402821399
pub static DISCRIMINATOR_BONDING_CURVE: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
// create discriminator 8576854823835016728
pub static DISCRIMINATOR_CREATE: u64 = 8576854823835016728;
// buy discriminator 16927863322537952870
pub static DISCRIMINATOR_BUY: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
// sell discriminator 12502976635542562355
pub static DISCRIMINATOR_SELL: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

pub fn load_private_key() -> Result<[u8; 64]> {
    if DRY_RUN {
        // Throwaway keypair: dry runs never sign real transactions
        return Ok(solana_sdk::signature::Keypair::new().to_bytes());
    }

    let key_file = fs::read_to_string("key.json")?;
    let key_vec: Vec<u8> = serde_json::from_str(&key_file)?;
    
    if key_vec.len() != 64 {
        return Err(anyhow::anyhow!("Invalid private key length"));
    }
    
    let mut key_array = [0u8; 64];
    key_array.copy_from_slice(&key_vec);
    Ok(key_array)
}
