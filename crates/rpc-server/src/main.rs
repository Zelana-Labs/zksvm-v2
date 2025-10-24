mod error;
mod routes;
mod state;

use rollup_core::{
    db::{reconcile_databases_on_startup,Storage},
    sequencer::RollupCore,
    types::{Account, Pubkey, Signature, Transaction, TransactionType},
};
use state::AppState;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;


#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>>{
    let temp_dir = tempdir()?;
    let rocks_path = temp_dir.path().join("rocksdb");
    let sqlite_path = temp_dir.path().join("checkpoints.db");
    let storage = Arc::new(Storage::new(
        rocks_path.to_str().unwrap(),
        sqlite_path.to_str().unwrap()
    ).await?);

    reconcile_databases_on_startup(&storage).await?;
    println!("[Main] Storage initialized and reconciled.");


    // intiialize and run rollupcore 
    let (tx_sender, tx_receiver) = mpsc::channel(100);
    let core_storage = Arc::clone(&storage);
    let rollup_core = RollupCore::new(core_storage, tx_receiver).await?;
    let core_handle = tokio::spawn(rollup_core.run());
    println!("[Main] RollupCore service started in the background.");

    // This is a placeholder for genesis account creation.
    // In a real app, this would be handled by a genesis file loader.
    if storage.rocksdb.iterator_cf(storage.cf_batches(), rocksdb::IteratorMode::Start).next().is_none() {
        let mut initial_batch = rocksdb::WriteBatch::default();
        let cf = storage.cf_accounts();
        initial_batch.put_cf(cf, bincode::serialize(&Pubkey::new([1;32]))?, bincode::serialize(&Account{balance: 1_000_000, nonce: 0})?);
        storage.rocksdb.write(initial_batch)?;
        println!("[Main] Genesis accounts populated.");
    }

    // Start the RPC Server 
    let rpc_state = AppState { storage , tx_sender:tx_sender.clone()};
    let app = routes::create_router(rpc_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("[RPC] Server listening on 0.0.0.0:3000");
    let rpc_handle = tokio::spawn(async move {
        println!("[RPC] Server started.");
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("[RPC] Server error: {}", err);
        }
    });

    // Simulate sending transactions (for testing) ---
    // In a real system, these would come from the RPC write endpoints or the L1 listener.
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // Give server a moment to start
    println!("[Main] Simulating incoming transactions...");
    tx_sender.send(Transaction {
        sender: Pubkey::new([1; 32]),
        recipient: Pubkey::new([2; 32]),
        tx_type: TransactionType::Transfer { amount: 100 },
        signature: Signature([0; 32]),
    }).await?;
    
    println!("\nServer is running. Try the following commands:");
    println!("\nAll services are running. You can now send transactions to the RPC server.");
    println!("Example using curl:");
    println!(r#"  curl -X POST http://127.0.0.1:3000/v1/send_transaction \
  -H "Content-Type: application/json" \
  -d '{{
    "sender": "0101010101010101010101010101010101010101010101010101010101010101",
    "recipient": "0202020202020202020202020202020202020202020202020202020202020202",
    "tx_type": {{ "Transfer": {{ "amount": 10 }} }},
    "signature": "0000000000000000000000000000000000000000000000000000000000000000"
}}'"#);
    // Wait for the servers to finish (which they won't, unless there's an error)
    tokio::select! {
        _ = core_handle => eprintln!("[Main] RollupCore unexpectedly shut down."),
        _ = rpc_handle => eprintln!("[Main] RPC Server unexpectedly shut down."),
    }

    Ok(())
}