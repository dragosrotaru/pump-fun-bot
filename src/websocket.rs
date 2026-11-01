use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use solana_sdk::transaction::VersionedTransaction;
use anyhow::Result;
use anyhow::anyhow;
use std::time::{Duration, Instant};
use solana_program::instruction::CompiledInstruction;
use crate::config;
use crate::logger::Logger;
use base64::engine::general_purpose;
use bincode;
use base64::Engine;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::mq::TokenPublisher;
use std::collections::VecDeque;
use tokio::select;
use tokio::time::interval;

const RECENT_TOKENS_BUFFER_SIZE: usize = 10;

#[derive(Debug, Serialize)]
struct SubscriptionRequest {
    jsonrpc: String,
    id: i32,
    method: String,
    params: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateTokenArgs {
    pub mint: String,
    pub bonding_curve: String,
    pub associated_bonding_curve: String,
    pub user: String,
    pub name: String,
    pub symbol: String,
}

struct WebSocketState {
    write: futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    last_ping: Instant,
}

pub struct WebSocketClient<'a> {
    url: String,
    connection: Arc<Mutex<Option<WebSocketState>>>,
    publisher: Arc<Mutex<TokenPublisher>>,
    logger: Logger<'a>,
    recent_tokens: Arc<Mutex<VecDeque<String>>>,  // Buffer to store recent token mints
}

impl<'a> WebSocketClient<'a> {
    pub fn new(logger: Logger<'a>) -> Result<Self> {
        let url = config::WSS_ENDPOINTS[0].to_string();
        Ok(Self { 
            url,
            connection: Arc::new(Mutex::new(None)),
            publisher: Arc::new(Mutex::new(TokenPublisher::new()?)),
            logger,
            recent_tokens: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_TOKENS_BUFFER_SIZE))),
        })
    }

    async fn ensure_connected(&self) -> Result<()> {
        let mut conn_guard = self.connection.lock().await;
        
        // Check if we need to establish a new connection
        if conn_guard.is_none() {
            println!("Connecting to WebSocket...");
            println!("{}", self.url);
            let (ws_stream, _) = connect_async(&self.url).await?;
            println!("Connected to WebSocket");
            let (write, read) = ws_stream.split();
            
            let mut state = WebSocketState {
                write,
                read,
                last_ping: Instant::now(),
            };

            // Subscribe to program transactions
            let subscription = SubscriptionRequest {
                jsonrpc: "2.0".to_string(),
                id: 1,
                method: "blockSubscribe".to_string(),
                params: vec![
                    json!({
                        "mentionsAccountOrProgram": config::PUMP_PROGRAM_PUBKEY.to_string()
                    }),
                    json!({
                        "commitment": "confirmed",
                        "encoding": "base64",
                        "showRewards": false,
                        "transactionDetails": "full",
                        "maxSupportedTransactionVersion": 0
                    }),
                ],
            };

            println!("Subscribing to pump program transactions...");
            let subscription_str = serde_json::to_string(&subscription)?;
            println!("{}", subscription_str);

            state.write.send(Message::Text(subscription_str.into())).await?;
            
            *conn_guard = Some(state);
        }
        
        Ok(())
    }

    async fn handle_ping(&self, state: &mut WebSocketState) -> Result<()> {
        let now = Instant::now();
        if now.duration_since(state.last_ping) >= Duration::from_secs(5) {
            state.write.send(Message::Ping(Vec::new().into())).await?;
            state.last_ping = now;
        }
        Ok(())
    }

    pub async fn listen(&self) -> Result<()> {
        self.ensure_connected().await?;
        let mut status_interval = interval(Duration::from_millis(100)); // Check status every 100ms
        
        loop {
            let mut conn_guard = self.connection.lock().await;
            let state = conn_guard.as_mut().ok_or_else(|| anyhow!("No connection"))?;
            
            select! {
                // Handle WebSocket messages
                msg = state.read.next() => {
                    if let Some(msg) = msg {
                        let msg = msg?;
                        if let Message::Text(text) = msg {
                            let value: Value = serde_json::from_str(&text)?;
                            
                            if let Some(params) = value.get("params") {
                                if let Some(result) = params.get("result") {
                                    if let Some(block) = result.get("value").and_then(|v| v.get("block")) {
                                        if let Some(transactions) = block.get("transactions").and_then(|t| t.as_array()) {
                                            for tx in transactions {
                                                if let Err(e) = self.process_transaction(tx).await {
                                                    eprintln!("Error processing transaction: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        drop(conn_guard);
                    } else {
                        // Connection was closed
                        *conn_guard = None;
                        drop(conn_guard);
                        
                        // Try to reconnect
                        self.ensure_connected().await?;
                    }
                },
                
                // Check worker status periodically
                _ = status_interval.tick() => {
                    // Handle ping if needed
                    self.handle_ping(state).await?;
                    
                    // Check worker status
                    let mut publisher = self.publisher.lock().await;
                    publisher.check_worker_status()?;
                    drop(publisher);
                    drop(conn_guard);
                },
            }
        }
    }

    async fn process_transaction(&self, tx: &Value) -> Result<()> {
        if let Some(tx_data) = tx.get("transaction") {
            let tx_bytes = general_purpose::STANDARD.decode(&tx_data[0].as_str().unwrap())?;
            let transaction = bincode::deserialize::<VersionedTransaction>(&tx_bytes)?;

            for ix in transaction.message.instructions() {
                if transaction.message.static_account_keys()[ix.program_id_index as usize] == config::PUMP_PROGRAM_PUBKEY {
                    let ix_data = &ix.data;
                    let discriminator = u64::from_le_bytes(ix_data[..8].try_into()?);

                    if discriminator == config::DISCRIMINATOR_CREATE {
                        let args = self.decode_create_instruction(ix_data, &transaction, ix)?;
                        
                        // Check for duplicates in the recent tokens buffer
                        let mut recent_tokens = self.recent_tokens.lock().await;
                        if recent_tokens.contains(&args.mint) {
                            println!("Duplicate token found: {}", args.name);
                            continue;
                        }

                        // Add the new token mint to the buffer
                        if recent_tokens.len() >= RECENT_TOKENS_BUFFER_SIZE {
                            recent_tokens.pop_front();
                        }
                        recent_tokens.push_back(args.mint.clone());
                        drop(recent_tokens);

                        println!("Publishing Token {}", args.name);
                        let mut publisher = self.publisher.lock().await;
                        publisher.publish(args.clone())?;
                        self.logger.log_token(args.clone()).await?;
                    }
                }
            }
        }
        Ok(())
    }

    fn decode_create_instruction(
        &self,
        data: &[u8],
        transaction: &VersionedTransaction,
        ix: &CompiledInstruction,
    ) -> Result<CreateTokenArgs> {
        let mut offset = 8; // Skip discriminator

        // Read string length
        let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into()?);
        offset += 4;
        
        // Read name
        let name = String::from_utf8(data[offset..offset + name_len as usize].to_vec())?;
        offset += name_len as usize;

        // Read symbol length
        let symbol_len = u32::from_le_bytes(data[offset..offset + 4].try_into()?);
        offset += 4;
        
        // Read symbol
        let symbol = String::from_utf8(data[offset..offset + symbol_len as usize].to_vec())?;

        let account_keys = transaction.message.static_account_keys();
        
        Ok(CreateTokenArgs {
            mint: account_keys[ix.accounts[0] as usize].to_string(),
            bonding_curve: account_keys[ix.accounts[2] as usize].to_string(),
            associated_bonding_curve: account_keys[ix.accounts[3] as usize].to_string(),
            user: account_keys[ix.accounts[7] as usize].to_string(),
            name,
            symbol,
        })
    }
} 