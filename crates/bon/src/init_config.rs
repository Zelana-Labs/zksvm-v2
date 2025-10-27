use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
    signer,
    system_program,
    transaction::Transaction,
};
use std::str::FromStr;
use dotenvy::dotenv;

// ====================================================================
// TODO: UPDATE THIS VALUE BEFORE RUNNING (if needed)
// ====================================================================
const BRIDGE_PROGRAM_ID: &str = "95sWqtU9fdm19cvQYu94iKijRuYAv3wLqod1pcsSfYth"; // Your Program ID
// ====================================================================

const CONFIG_SEED: &[u8] = b"config";
const VAULT_SEED: &[u8] = b"vault"; // Seed for the vault PDA

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file to get KEYPAIR1 (this will be our admin/payer)
    dotenv().ok();
    let keypair_path = std::env::var("KEYPAIR1")?;
    let payer = signer::keypair::read_keypair_file(keypair_path)
        .map_err(|e| anyhow::anyhow!("Failed to read keypair file: {}", e))?;

    println!("Loaded Payer/Admin keypair: {}", payer.pubkey());

    // Connect to Solana Devnet
    let rpc_client = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".into(),
        CommitmentConfig::confirmed(),
    );

    let bridge_program_id = Pubkey::from_str(BRIDGE_PROGRAM_ID)?;

    // --- 1. Define Config Account Data ---
    // Use the payer's keypair as the sequencer authority for testing
    let sequencer_authority = payer.pubkey();
    let domain: [u8; 32] = [1; 32]; // Simple domain separator

    println!("Attempting to initialize bridge config...");
    println!("Using Bridge Program: {}", bridge_program_id);
    println!("Setting Sequencer Authority to: {}", sequencer_authority);

    // --- 2. Derive PDAs ---
    let (config_pda, _config_bump) = Pubkey::find_program_address(
        &[CONFIG_SEED],
        &bridge_program_id
    );

    // Vault PDA seeds: ["vault", config.key]
    let (vault_pda, _vault_bump) = Pubkey::find_program_address(
        &[VAULT_SEED, config_pda.as_ref()], // Vault depends on Config PDA
        &bridge_program_id
    );

    println!("Derived Config PDA: {}", config_pda);
    println!("Derived Vault PDA: {}", vault_pda);

    // --- 3. Build Pinocchio Instruction Data ---
    // Discriminator = 1 (for init_config based on your program/src/instruction/mod.rs)
    // Data = InitParams { sequencer_authority, domain }
    let mut instruction_data: Vec<u8> = vec![];
    instruction_data.push(0); // <-- Discriminator for 'Initialize' is 0 in your IDL and entrypoint.rs
    instruction_data.extend_from_slice(sequencer_authority.as_ref());
    instruction_data.extend_from_slice(&domain);

    // --- 4. Build Account Metas (CORRECT ORDER and FLAGS) ---
    // Order MUST match program/src/instruction/init.rs
    let accounts = vec![
        // 0. [signer, writable] Payer (pays for accounts) - CORRECT
        AccountMeta::new(payer.pubkey(), true),

        // 1. [writable] Config PDA (gets created) - CORRECT
        AccountMeta::new(config_pda, false), // Marked writable

        // 2. [writable] Vault PDA (gets created) - CORRECT
        AccountMeta::new(vault_pda, false),  // Marked writable

        // 3. [] System Program (for the account creation) - CORRECT
        AccountMeta::new_readonly(system_program::id(), false),
    ];


    // --- 5. Create and Send Transaction ---
    let instruction = Instruction {
        program_id: bridge_program_id,
        accounts,
        data: instruction_data,
    };

    let recent_blockhash = rpc_client.get_latest_blockhash().await?;

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer], // Only the payer needs to sign this TX
        recent_blockhash,
    );

    println!("Sending init_config transaction...");

    match rpc_client.send_and_confirm_transaction(&transaction).await {
        Ok(signature) => {
            println!("✅ Bridge Config Initialized Successfully!");
            println!("Transaction Signature: {}", signature);
            println!("View on Solana Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
            println!("\n========================================================");
            println!("✅ Your Config Account Pubkey is: {}", config_pda);
            println!("========================================================\n");
            println!("COPY THIS PUBKEY and paste it into the 'CONFIG_PUBKEY' constant in your 'test_deposit.rs' file.");
        }
        Err(e) => {
            println!("❌ Transaction failed: {}", e);
            // Optionally print simulation logs if available
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