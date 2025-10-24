use axum::{response::Json, serve};
use rollup_core::{
    db::{reconcile_databases_on_startup, Storage},
    sequencer::RollupCore,
    types::{Account, BlockHeader, Pubkey, Signature, Transaction, TransactionType},
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;
use rpc_server::{routes::create_router, state::AppState};

async fn spawn_app()->(SocketAddr,mpsc::Sender<Transaction>,Arc<Storage>){
    let temp_dir = tempdir().unwrap();
    let rocks_path = temp_dir.path().join("rocksdb");
    let sqlite_path = temp_dir.path().join("checkpoints.db");

    let storage = Arc::new(
        Storage::new(
            rocks_path.to_str().unwrap(),
            sqlite_path.to_str().unwrap(),
        )
        .await
        .unwrap(),
    );
    reconcile_databases_on_startup(&storage).await.unwrap();

    let (tx_sender, tx_receiver) = mpsc::channel(100);

    let core_storage = Arc::clone(&storage);
    let rollup_core = RollupCore::new(core_storage, tx_receiver).await.unwrap();
    tokio::spawn(rollup_core.run());

    let rpc_state = AppState {
        storage: Arc::clone(&storage),
    };

    let port = portpicker::pick_unused_port().expect("No free ports");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let app = create_router(rpc_state);
    tokio::spawn(serve(listener, app).into_future());

    (addr, tx_sender, storage)
}


#[tokio::test]
async fn test_happy_path_genesis_and_first_block() {
   let (addr, tx_sender, storage) = spawn_app().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    assert_eq!(client.get(format!("{}/v1/tip", base_url)).send().await.unwrap().status(), 404);

    let mut initial_batch = rocksdb::WriteBatch::default();
    let (acc1_pk, acc2_pk) = (Pubkey::new([1; 32]), Pubkey::new([2; 32]));
    initial_batch.put_cf(storage.cf_accounts(), &acc1_pk.0, &bincode::serialize(&Account { balance: 1000, nonce: 0 }).unwrap());
    storage.rocksdb.write(initial_batch).unwrap();
    
    // FIX: Use a 32-byte signature
    let tx = Transaction {
        sender: acc1_pk,
        recipient: acc2_pk,
        tx_type: TransactionType::Transfer { amount: 100 },
        signature: Signature([5; 32]),
    };
    tx_sender.send(tx.clone()).await.unwrap();
    for i in 0..4 {
        tx_sender.send(Transaction { sender: acc1_pk, recipient: acc2_pk, tx_type: TransactionType::Transfer { amount: 1 }, signature: Signature([i; 32]) }).await.unwrap();
    }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let res = client.get(format!("{}/v1/tip", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let tip: Value = res.json().await.unwrap();
    assert_eq!(tip["batch_id"], 1);

    let res = client.get(format!("{}/v1/accounts/{}", base_url, hex::encode(acc1_pk.0))).send().await.unwrap();
    let account: Account = res.json().await.unwrap();
    assert_eq!(account.balance, 896);
    assert_eq!(account.nonce, 5);

    // FIX: URL uses 64-char hex string for the 32-byte signature
    let res = client.get(format!("{}/v1/tx/{}", base_url, hex::encode(tx.signature.0))).send().await.unwrap();
    assert_eq!(res.status(), 200);

    let res = client.get(format!("{}/v1/batches/1", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let header: Value = res.json().await.unwrap();
    assert_eq!(header["batch_id"], 1);
}


#[tokio::test]
async fn test_bad_input_validation() {
    let (addr, _, _) = spawn_app().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    // Test account with invalid hex
    let res = client.get(format!("{}/v1/accounts/not-a-hex-string", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 400);
    let error: Value = res.json().await.unwrap();
    assert_eq!(error["error"]["code"], "bad_request");

    // Test account with wrong length
    let res = client.get(format!("{}/v1/accounts/010203", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 400);

    // Test tx with wrong length
    let res = client.get(format!("{}/v1/tx/010203", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 400);
}