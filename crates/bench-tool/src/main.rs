use rollup_core::{
    db::Storage,
    sequencer::{commit_batch, compute_state_commitment},
    types::{Account, BlockHeader, Pubkey, Signature, Transaction, TransactionType},
};
use std::{collections::{BTreeMap, HashMap}, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use rand::{rngs::StdRng, Rng, SeedableRng};
use indicatif::{ProgressBar, ProgressStyle};

const NUM_ACCOUNTS: u64 = 100_000;
const NUM_BLOCKS: u64 = 10_000;
const TX_PER_BLOCK: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Database Seeding Tool");
    let storage_dir = std::env::current_dir()?.join("test_db");
    if storage_dir.exists() {
        println!("Removing existing test database at {:?}", storage_dir);
        std::fs::remove_dir_all(&storage_dir)?;
    }
    std::fs::create_dir(&storage_dir)?;
    
    let rocks_path = storage_dir.join("rocksdb");
    let sqlite_path = storage_dir.join("checkpoints.db");

    let storage = Arc::new(Storage::new(
        rocks_path.to_str().unwrap(),
        sqlite_path.to_str().unwrap(),
    ).await?);
    println!("Database initialized at {:?}", storage_dir);

    let mut rng = StdRng::seed_from_u64(42);
    let mut tip = BlockHeader::genesis();
    let mut all_accounts = BTreeMap::new();

    println!("\nSeeding {} initial accounts...", NUM_ACCOUNTS);
    let pb = ProgressBar::new(NUM_ACCOUNTS);
    pb.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?);

    let mut initial_batch = rocksdb::WriteBatch::default();
    let cf = storage.cf_accounts();
    for _ in 0..NUM_ACCOUNTS {
        let pubkey = Pubkey::new(rng.random());
        let account = Account { balance: 1_000_000, nonce: 0 };
        initial_batch.put_cf(cf, &pubkey.0, &bincode::serialize(&account)?);
        all_accounts.insert(pubkey, account);
        pb.inc(1);
    }
    storage.rocksdb.write(initial_batch)?;
    pb.finish_with_message("done");

    println!("\nSimulating and committing {} blocks...", NUM_BLOCKS);
    let pb_blocks = ProgressBar::new(NUM_BLOCKS);
    pb_blocks.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?);

    let account_pks: Vec<Pubkey> = all_accounts.keys().cloned().collect();

    for _ in 0..NUM_BLOCKS {
        let mut write_set = HashMap::new();
        let mut transactions = Vec::new();

        for _ in 0..TX_PER_BLOCK {
            let sender_pk = account_pks[rng.random_range(0..account_pks.len())];
            let recipient_pk = account_pks[rng.random_range(0..account_pks.len())];
            let mut sender = all_accounts.get(&sender_pk).unwrap().clone();
            
            if sender.balance > 1 {
                sender.balance -= 1;
                sender.nonce += 1;
                write_set.insert(sender_pk, sender);

                transactions.push(Transaction {
                    sender: sender_pk,
                    recipient: recipient_pk,
                    tx_type: TransactionType::Transfer { amount: 1 },
                    signature: Signature(rng.random()),
                });
            }
        }
        
        all_accounts.extend(write_set.clone());
        let new_root = compute_state_commitment(&all_accounts, tip.batch_id + 1);
        
        let header = BlockHeader {
            batch_id: tip.batch_id + 1,
            prev_root: tip.new_root,
            new_root,
            tx_count: transactions.len() as u32,
            open_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ..BlockHeader::genesis()
        };

        commit_batch(&storage, &header, &write_set, &transactions).await?;
        tip = header;
        pb_blocks.inc(1);
    }
    pb_blocks.finish_with_message("done");
    
    println!("\n--- Seeding Complete ---");
    Ok(())
}

