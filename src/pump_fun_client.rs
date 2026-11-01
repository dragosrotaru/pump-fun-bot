use anyhow::{anyhow, Result};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use uuid::Uuid;
use crate::logger::Logger;
use crate::values::{SolValue, TokenValue};
use crate::timer::TimerSeq;
use crate::config;
use crate::curve_state::{CurveState, CurveHistory};
use crate::db::Strategy;

pub struct PumpFunClient<'a> {
    pub client: RpcClient,
    pub payer: Keypair,
    pub logger: &'a Logger<'a>,
    current_rpc_index: usize,
}

impl<'a> PumpFunClient<'a> {
    pub async fn new(
        logger: &'a Logger<'a>,
    ) -> Result<Self> {
        let private_key = config::load_private_key()?;
        let rpc_url = &config::RPC_ENDPOINTS[0];
        let client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        let payer = Keypair::from_bytes(&private_key).unwrap();

        Ok(Self {
            client,
            payer,
            logger,
            current_rpc_index: 0,
        })
    }

    pub fn try_next_rpc_endpoint(&mut self) -> Result<()> {
        self.current_rpc_index = (self.current_rpc_index + 1) % config::RPC_ENDPOINTS.len();
        let rpc_url = &config::RPC_ENDPOINTS[self.current_rpc_index];
        self.client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        println!("Switching to RPC endpoint: {}", rpc_url);
        Ok(())
    }

    /**
     * returns a create associated token account instruction
     */
    pub fn create_asa_instruction(&self, mint: &Pubkey) -> Instruction {
        return spl_associated_token_account::instruction::create_associated_token_account(
            &self.payer.pubkey(),
            &self.payer.pubkey(),
            &mint,
            &spl_token::id(),
        );
    }

    /**
     * returns a buy token instruction
     */
    pub fn buy_token_instruction(
        &self,
        mint: &Pubkey,
        bonding_curve: &Pubkey,
        associated_bonding_curve: &Pubkey,
        associated_token_account: &Pubkey,
        tokens_to_receive: TokenValue,
        max_spend: SolValue,
        timer: &mut TimerSeq,
    ) -> Instruction {
        timer.next("create_instruction");
        let accounts = vec![
            AccountMeta::new_readonly(config::PUMP_GLOBAL_PUBKEY, false),
            AccountMeta::new(config::PUMP_FEE_PUBKEY, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*bonding_curve, false),
            AccountMeta::new(*associated_bonding_curve, false),
            AccountMeta::new(*associated_token_account, false),
            AccountMeta::new(self.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new_readonly(config::PUMP_EVENT_AUTHORITY_PUBKEY, false),
            AccountMeta::new_readonly(config::PUMP_PROGRAM_PUBKEY, false),
        ];

        let mut data = Vec::with_capacity(24);
        data.extend_from_slice(&config::DISCRIMINATOR_BUY);
        data.extend_from_slice(&tokens_to_receive.integer.to_le_bytes());
        data.extend_from_slice(&max_spend.lamports.to_le_bytes());

        Instruction {
            program_id: config::PUMP_PROGRAM_PUBKEY,
            accounts,
            data,
        }
    }

    /**
     * returns a sell token instruction
     */
    pub fn sell_token_instruction(
        &self,
        mint: &Pubkey,
        bonding_curve: &Pubkey,
        associated_bonding_curve: &Pubkey,
        associated_token_account: &Pubkey,
        token_value: TokenValue,
        min_output: SolValue,
        timer: &mut TimerSeq,
    ) -> Instruction {
        timer.next("create_instruction");
        let accounts = vec![
            AccountMeta::new_readonly(config::PUMP_GLOBAL_PUBKEY, false),
            AccountMeta::new(config::PUMP_FEE_PUBKEY, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*bonding_curve, false),
            AccountMeta::new(*associated_bonding_curve, false),
            AccountMeta::new(*associated_token_account, false),
            AccountMeta::new(self.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(config::PUMP_EVENT_AUTHORITY_PUBKEY, false),
            AccountMeta::new_readonly(config::PUMP_PROGRAM_PUBKEY, false),
        ];

        let mut data = Vec::with_capacity(24);
        data.extend_from_slice(&config::DISCRIMINATOR_SELL);
        data.extend_from_slice(&token_value.integer.to_le_bytes());
        data.extend_from_slice(&min_output.lamports.to_le_bytes());

        Instruction {
            program_id: config::PUMP_PROGRAM_PUBKEY,
            accounts,
            data,
        }
    }

    pub fn send_buy_transaction(
        &mut self,
        create_asa_instruction: Instruction,
        buy_instruction: Instruction,
        timer: &mut TimerSeq,
        strategy: &Strategy,
        corr_id: Uuid,
    ) -> Result<String> {
        timer.next("get_latest_blockhash");

        let mut attempts = 0;
        let max_attempts = config::RPC_ENDPOINTS.len();
        
        // Clone instructions before the loop
        let create_asa_ix = create_asa_instruction;
        let buy_ix = buy_instruction;
        
        loop {
            // Get latest blockhash
            let recent_blockhash = match self.client.get_latest_blockhash() {
                Ok(blockhash) => blockhash,
                Err(err) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!("All RPC endpoints failed to get blockhash: {}", err));
                    }
                    println!("Failed to get blockhash: {}. Trying next endpoint...", err);
                    self.try_next_rpc_endpoint()?;
                    continue;
                }
            };

            timer.next("create_transaction");

            // Create compute budget instructions with calculated limit
            let compute_budget_ix =
                solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(
                    strategy.buy_compute_units as u32,
                );
            let priority_fees_ix =
                solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                    strategy.buy_compute_price_mlamports as u64,
                );

            let transaction = Transaction::new_signed_with_payer(
                &[
                    compute_budget_ix,
                    priority_fees_ix,
                    create_asa_ix.clone(),
                    buy_ix.clone(),
                ],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                recent_blockhash,
            );

            timer.next("send_transaction");

            if !config::DRY_RUN {
                match self.client.send_and_confirm_transaction_with_spinner_and_config(
                    &transaction,
                    CommitmentConfig::confirmed(),
                    RpcSendTransactionConfig {
                        skip_preflight: true,
                        preflight_commitment: Some(CommitmentLevel::Confirmed),
                        ..RpcSendTransactionConfig::default()
                    },
                ) {
                    Ok(sig) => {
                        println!(
                            "Buy Transaction: https://explorer.solana.com/tx/{}",
                            transaction.signatures[0]
                        );
                        return Ok(sig.to_string());
                    }
                    Err(err) => {
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(anyhow!("All RPC endpoints failed to send transaction: {}", err));
                        }
                        println!("Failed to send transaction: {}. Trying next endpoint...", err);
                        self.try_next_rpc_endpoint()?;
                        continue;
                    }
                }
            } else {
                // Use a dummy signature for dry run
                match self.simulate_transaction(transaction.clone()) {
                    Ok(_) => {
                        return Ok(solana_sdk::signature::Signature::default().to_string()
                            + "_buy_"
                            + corr_id.to_string().as_str());
                    }
                    Err(err) => {
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(anyhow!("All RPC endpoints failed to simulate transaction: {}", err));
                        }
                        println!("Failed to simulate transaction: {}. Trying next endpoint...", err);
                        self.try_next_rpc_endpoint()?;
                        continue;
                    }
                }
            }
        }
    }

    pub fn send_sell_transaction(
        &mut self,
        sell_instruction: Instruction,
        timer: &mut TimerSeq,
        strategy: &Strategy,
        corr_id: Uuid,
    ) -> Result<String> {
        timer.next("get_latest_blockhash");
        
        let mut attempts = 0;
        let max_attempts = config::RPC_ENDPOINTS.len();
        
        // Clone instruction before the loop
        let sell_ix = sell_instruction;
        
        loop {
            let recent_blockhash = match self.client.get_latest_blockhash() {
                Ok(blockhash) => blockhash,
                Err(err) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!("All RPC endpoints failed to get blockhash: {}", err));
                    }
                    println!("Failed to get blockhash: {}. Trying next endpoint...", err);
                    self.try_next_rpc_endpoint()?;
                    continue;
                }
            };

            timer.next("create_transaction");

            // Create compute budget instructions with calculated limit
            let compute_budget_ix =
                solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(
                    strategy.sell_compute_units as u32,
                );
            let priority_fees_ix =
                solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                    strategy.sell_compute_price_mlamports as u64,
                );

            let transaction = Transaction::new_signed_with_payer(
                &[compute_budget_ix, priority_fees_ix, sell_ix.clone()],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                recent_blockhash,
            );

            timer.next("send_transaction");

            if !config::DRY_RUN {
                match self.client.send_and_confirm_transaction_with_spinner_and_config(
                    &transaction,
                    CommitmentConfig::confirmed(),
                    RpcSendTransactionConfig {
                        skip_preflight: true,
                        preflight_commitment: Some(CommitmentLevel::Confirmed),
                        ..RpcSendTransactionConfig::default()
                    },
                ) {
                    Ok(sig) => {
                        println!(
                            "Sell Transaction: https://explorer.solana.com/tx/{}",
                            transaction.signatures[0]
                        );
                        return Ok(sig.to_string());
                    }
                    Err(err) => {
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(anyhow!("All RPC endpoints failed to send transaction: {}", err));
                        }
                        println!("Failed to send transaction: {}. Trying next endpoint...", err);
                        self.try_next_rpc_endpoint()?;
                        continue;
                    }
                }
            } else {
                match self.simulate_transaction(transaction.clone()) {
                    Ok(_) => {
                        return Ok(solana_sdk::signature::Signature::default().to_string()
                            + "_sell_"
                            + corr_id.to_string().as_str());
                    }
                    Err(err) => {
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(anyhow!("All RPC endpoints failed to simulate transaction: {}", err));
                        }
                        println!("Failed to simulate transaction: {}. Trying next endpoint...", err);
                        self.try_next_rpc_endpoint()?;
                        continue;
                    }
                }
            }
        }
    }

    /**
     * Returns a tuple of [token_count, max_amount_lamports]
     */
    pub async fn buy_token(
        &mut self,
        mint: &Pubkey,
        bonding_curve: &Pubkey,
        associated_bonding_curve: &Pubkey,
        associated_token_account: &Pubkey,
        buy_amount: SolValue,
        curve_state: &mut CurveState,
        corr_id: Uuid,
        strategy: &Strategy,
    ) -> Result<(TokenValue, SolValue)> {
        let mut timer = TimerSeq::start("buy_token");
        // Get curve state and calculate price
        // todo should we be including the slippage in the buy_tokens function?
        let tokens_received = curve_state.buy_tokens(buy_amount)?;
        let max_spend = SolValue::from_sol(buy_amount.sol * (1.0 + strategy.buy_slippage));

        println!(
            "Buying: {} SOL • {} tokens • {:.20} SOL/token",
            buy_amount.sol, tokens_received.float, curve_state.price.sol
        );

        // Instructions
        let create_asa_instruction = self.create_asa_instruction(&mint);
        let buy_instruction = self.buy_token_instruction(
            &mint,
            bonding_curve,
            associated_bonding_curve,
            associated_token_account,
            tokens_received,
            max_spend,
            &mut timer,
        );

        let sig = self.send_buy_transaction(
            create_asa_instruction,
            buy_instruction,
            &mut timer,
            strategy,
            corr_id,
        )?;

        // Log timing and trade
        self.logger.log_timing(corr_id, "buy", &mut timer).await?;
        self.logger
            .log_trade(
                corr_id,
                "buy",
                &mint,
                curve_state.price,
                tokens_received,
                &sig.to_string(),
            )
            .await?;

        Ok((tokens_received, max_spend))
    }

    pub async fn sell_token(
        &mut self,
        mint: &Pubkey,
        bonding_curve: &Pubkey,
        associated_bonding_curve: &Pubkey,
        associated_token_account: &Pubkey,
        corr_id: Uuid,
        token_value: TokenValue,
        curve_state: &CurveState,
        strategy: &Strategy,
    ) -> Result<String> {
        let mut timer = TimerSeq::start("sell_token");
        // Calculate minimum SOL output
        let min_output = SolValue::from_sol(
            token_value.float * curve_state.price.sol * (1.0 - strategy.sell_slippage),
        );
        println!(
            "Selling: {} tokens • {:.20} SOL/token",
            token_value.float, curve_state.price.sol
        );

        let sell_instruction = self.sell_token_instruction(
            &mint,
            bonding_curve,
            associated_bonding_curve,
            associated_token_account,
            token_value,
            min_output,
            &mut timer,
        );

        let signature =
            self.send_sell_transaction(sell_instruction, &mut timer, strategy, corr_id)?;

        // Log timing and trade
        self.logger.log_timing(corr_id, "sell", &mut timer).await?;
        self.logger
            .log_trade(
                corr_id,
                "sell",
                &mint,
                curve_state.price,
                token_value,
                &signature,
            )
            .await?;

        Ok(signature.to_string())
    }

    /**
     * Fetches the token balance for a given token account
     */
    pub async fn get_token_balance(&mut self, token_account: &Pubkey) -> Result<TokenValue> {
        let mut attempts = 0;
        let max_attempts = config::RPC_ENDPOINTS.len();
        
        loop {
            match self.client.get_token_account_balance(token_account) {
                Ok(account) => {
                    return Ok(TokenValue::from_integer(
                        account.amount.parse().unwrap_or(0),
                    ));
                }
                Err(err) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!("All RPC endpoints failed: {}", err));
                    }
                    println!("RPC call failed: {}. Trying next endpoint...", err);
                    self.try_next_rpc_endpoint()?;
                }
            }
        }
    }

    /**
     * Fetches the latest curve state for a token and adds it to the curve history
     */
    pub fn fetch_curve_state(
        &mut self,
        curve_address: &Pubkey,
        curve_history: &mut CurveHistory,
        timer: &mut TimerSeq,
    ) -> Result<CurveState> {
        timer.next("fetch_curve_state");
        
        let mut attempts = 0;
        let max_attempts = config::RPC_ENDPOINTS.len();
        
        loop {
            match self.client.get_account(curve_address) {
                Ok(account) => {
                    let data = account.data;
                    if data.len() < 8 || &data[..8] != config::DISCRIMINATOR_BONDING_CURVE {
                        return Err(anyhow!("Invalid curve state discriminator"));
                    }
                    let curve_state = CurveState::from_bytes(&data).unwrap();
                    curve_history.add_curve(&curve_state.clone());
                    return Ok(curve_state);
                }
                Err(err) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!("All RPC endpoints failed: {}", err));
                    }
                    println!("RPC call failed: {}. Trying next endpoint...", err);
                    self.try_next_rpc_endpoint()?;
                }
            }
        }
    }

    /**
     * Simulates a transaction and prints the logs
     */
    pub fn simulate_transaction(&mut self, transaction: Transaction) -> Result<()> {
        let simulation = self.client.simulate_transaction_with_config(
            &transaction,
            RpcSimulateTransactionConfig {
                sig_verify: true,
                replace_recent_blockhash: true,
                commitment: Some(CommitmentConfig::confirmed()),
                ..RpcSimulateTransactionConfig::default()
            },
        )?;

        if let Some(err) = simulation.value.err {
            return Err(anyhow!("Transaction simulation failed: {:?}", err));
        }

        // print logs
        println!("Simulation logs: {:?}", simulation.value.logs);

        Ok(())
    }
}
