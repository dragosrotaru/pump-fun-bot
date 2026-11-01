use anyhow::Result;
use hft::db::Database;
use hft::logger::Logger;
use hft::websocket::WebSocketClient;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Solana WebSocket");
    let db = Database::new().await?;
    let logger = Logger::new(&db).await?;
    let ws_client = WebSocketClient::new(logger)?;
    ws_client.listen().await?;
    
    Ok(())
}