use thiserror::Error;

#[derive(Error, Debug)]
pub enum PumpFunError {
    #[error("Invalid curve state: {0}")]
    InvalidCurveState(String),
    
    #[error("RPC error: {0}")]
    RpcError(#[from] solana_client::client_error::ClientError),
    
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
} 