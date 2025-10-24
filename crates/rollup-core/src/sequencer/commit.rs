use crate::db::Storage;
use crate::types::{Account,BlockHeader,Pubkey,Transaction};
use rocksdb::{WriteBatch,WriteOptions};
use std::collections::HashMap;
use chrono::Utc;

/// Atomically commits a finalized batch to the db
pub async fn commit_batch(
    storage:&Storage,
    header: &BlockHeader,
    write_set: &HashMap<Pubkey,Account>,
    transactions : &[Transaction]
) ->Result<(),Box<dyn std::error::Error>>{
    let mut batch = WriteBatch::default();
    
    for (pubkey,account) in write_set{
        batch.put_cf(storage.cf_accounts(), &pubkey.0, bincode::serialize(account)?);
    }

    for tx in transactions {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        batch.put_cf(storage.cf_txs(), &tx.signature.0, bincode::serialize(tx)?);
        
        let mut time_key = Vec::with_capacity(8+32);
        time_key.extend_from_slice(&timestamp.to_be_bytes());
        time_key.extend_from_slice(&tx.signature.0);
        batch.put_cf(storage.cf_tx_by_time(), time_key, &[]);

        let mut sender_key = Vec::with_capacity(32+8+32);
        sender_key.extend_from_slice(&tx.sender.0);
        sender_key.extend_from_slice(&timestamp.to_be_bytes());
        sender_key.extend_from_slice(&tx.signature.0);
        batch.put_cf(storage.cf_tx_by_sender(), sender_key, &[]);
    }

    batch.put_cf(storage.cf_batches(), header.batch_id.to_be_bytes(), header.to_bytes()?);

    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    storage.rocksdb.write_opt(batch, &write_opts)?;
    
    sqlx::query("INSERT OR REPLACE INTO batches (id, new_root, committed_at) VALUES (?, ?, ?)")
        .bind(header.batch_id as i64)
        .bind(&header.new_root.to_vec())
        .bind(&Utc::now().to_rfc3339())
        .execute(&storage.sqlite)
        .await?;
        
    println!("[Commit] Batch {} committed successfully.", header.batch_id);
    Ok(())
}