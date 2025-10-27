use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    native_token::{lamports_to_sol, LAMPORTS_PER_SOL},
    pubkey::Pubkey,
    signature::Signer,
    signer,
    system_program,
    transaction::Transaction,
};
use std::str::FromStr;
use dotenvy::dotenv;

// ====================================================================
// TODO: UPDATE THESE VALUES
// ====================================================================
// 1. Get this from your deploy command
const BRIDGE_PROGRAM_ID: &str = "95sWqtU9fdm19cvQYu94iKijRuYAv3wLqod1pcsSfYth";

// 2. Get this from running 'init_config'
const CONFIG_PUBKEY: &str = "W5mZAx6Suc5zRUGhH53wi3eqeCwMNTebHpqu3K4UEN3";
// ====================================================================

// Seed for the DepositReceipt PDA, from state/depositreceipt.rs
const RECEIPT_SEED: &[u8] = b"receipt";

// --- THIS IS THE FIX ---
// Add the derives that bytemuck requires
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod,bytemuck::Zeroable )]
struct DepositParams {
    pub amount: u64,
    pub nonce: u64,
}
// --- END FIX ---

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file to get KEYPAIR1
    dotenv().ok();
    // Use your default solana wallet as the payer/depositor
    let payer = signer::keypair::read_keypair_file(
        "/home/shanks/.config/solana/id.json"
    ).unwrap();
    
    println!("Loaded depositor keypair: {}", payer.pubkey());

    // Connect to Solana Devnet
    let rpc_client = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".into(),
        CommitmentConfig::confirmed(),
    );

    let bridge_program_id = Pubkey::from_str(BRIDGE_PROGRAM_ID)?;
    let config_pubkey = Pubkey::from_str(CONFIG_PUBKEY)?;
    
    let deposit_amount_lamports: u64 = (0.01 * LAMPORTS_PER_SOL as f64) as u64; // 0.01 SOL
    
    let nonce: u64 = 231; 


    println!(
        "Attempting to deposit {} SOL ({}) lamports with nonce {}...",
        lamports_to_sol(deposit_amount_lamports),
        deposit_amount_lamports,
        nonce
    );
    println!("Using Bridge Program: {}", bridge_program_id);
    println!("Using Config Account: {}", config_pubkey);

    
    // Vault PDA: ["vault", config.key]
    let (vault_pda, _vault_bump) = Pubkey::find_program_address(
        &[b"vault", config_pubkey.as_ref()],
        &bridge_program_id
    );

    // Receipt PDA: ["receipt", config.key, depositor.key, nonce_le]
    let (receipt_pda, _receipt_bump) = Pubkey::find_program_address(
        &[
            RECEIPT_SEED,
            config_pubkey.as_ref(), // <-- Matches your test logic
            payer.pubkey().as_ref(),
            &nonce.to_le_bytes()
        ],
        &bridge_program_id
    );
    
    println!("Derived Vault PDA: {}", vault_pda);
    println!("Derived Receipt PDA: {}", receipt_pda);

    // --- 2. Build Instruction Data (matching program/tests/deposit.rs) ---
    
    // Discriminator = 1 (for BridgeIx::DEPOSIT)
    let mut instruction_data: Vec<u8> = vec![1]; 
    
    let ix_data = DepositParams {
        amount: deposit_amount_lamports,
        nonce,
    };
    // Use bytemuck to serialize, just like your test
    instruction_data.extend_from_slice(bytemuck::bytes_of(&ix_data));

    // --- 3. Build Account Metas (matching program/tests/deposit.rs) ---
    let accounts = vec![
        // 0. [signer, writable] Payer (Depositor)
        AccountMeta::new(payer.pubkey(), true),

        // 1. [readonly] Config PDA
        AccountMeta::new_readonly(config_pubkey, false),

        // 2. [writable] Vault PDA
        AccountMeta::new(vault_pda, false),

        // 3. [writable] DepositReceipt PDA
        AccountMeta::new(receipt_pda, false),

        // 4. [readonly] System Program
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    // --- 4. Create and Send Transaction ---
    let instruction = Instruction {
        program_id: bridge_program_id,
        accounts,
        data: instruction_data,
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().await?;

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    println!("Sending deposit transaction...");

    match rpc_client.send_and_confirm_transaction(&transaction).await {
        Ok(signature) => {
            println!("✅ Deposit successful!");
            println!("Transaction Signature: {}", signature);
            println!("View on Solana Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
        }
        Err(e) => {
            println!("❌ Transaction failed: {}", e);
            if let solana_client::client_error::ClientErrorKind::RpcError(
                solana_client::rpc_request::RpcError::RpcResponseError { data, .. }
            ) = e.kind() {
                if let solana_client::rpc_request::RpcResponseErrorData::SendTransactionPreflightFailure(sim_err) = data {
                    if let Some(logs) = &sim_err.logs {
                        println!("\n--- Simulation Logs ---");
                        for log in logs {
                            println!("{}", log);
                        }
                        println!("--- End Simulation Logs ---");
                    }
                }
            }
            return Err(e.into());
        }
    }

    Ok(())
}