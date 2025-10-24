use super::{batch::BatchContext, commit::commit_batch, commitment::compute_state_commitment};
use crate::db::Storage;
use crate::types::{Account, BlockHeader, Transaction};
use rocksdb::IteratorMode;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::Receiver;

const MAX_TX_PER_BATCH: usize = 5;

pub struct RollupCore {
    storage: Arc<Storage>,
    tx_receiver: Receiver<Transaction>,
    mempool: Vec<Transaction>,
    tip: BlockHeader,
}

impl RollupCore {
    pub async fn new(storage: Arc<Storage>, tx_receiver: Receiver<Transaction>) -> Result<Self, Box<dyn std::error::Error>> {
        let tip = Self::load_tip(storage.as_ref()).await?;
        Ok(Self { storage, tx_receiver, mempool: Vec::new(), tip })
    }

    async fn load_tip(storage: &Storage) -> Result<BlockHeader, Box<dyn std::error::Error>> {
        Ok(storage.rocksdb.iterator_cf(storage.cf_batches(), IteratorMode::End)
            .next()
            .and_then(|res| res.ok())
            .map(|(_, val)| BlockHeader::from_bytes(val.as_ref().try_into().unwrap()).unwrap())
            .unwrap_or_else(BlockHeader::genesis))
    }

    pub async fn run(mut self) {
        println!("[Core] RollupCore started. Tip is at batch {}.", self.tip.batch_id);
        while let Some(tx) = self.tx_receiver.recv().await {
            self.mempool.push(tx);
            if self.mempool.len() >= MAX_TX_PER_BATCH {
                if let Err(e) = self.seal_and_commit_batch().await {
                    eprintln!("[Core] Failed to seal batch: {}", e);
                }
            }
        }
        if !self.mempool.is_empty() {
            if let Err(e) = self.seal_and_commit_batch().await {
                 eprintln!("[Core] Failed to seal final batch: {}", e);
            }
        }
        println!("[Core] Transaction channel closed. Shutting down.");
    }

    async fn seal_and_commit_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let txs_to_process = std::mem::take(&mut self.mempool);
        println!("[Core] Sealing batch {} with {} txs.", self.tip.batch_id + 1, txs_to_process.len());

        let mut batch_context = BatchContext::new(&self.storage);
        for tx in &txs_to_process {
            if let Err(e) = batch_context.execute_transaction(tx) {
                eprintln!("[Core] Tx failed: {:?}, Error: {}", tx.signature, e);
            }
        }
        
        let mut all_accounts = BTreeMap::new();
        let iter = self.storage.rocksdb.iterator_cf(self.storage.cf_accounts(), IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            all_accounts.insert(bincode::deserialize(&key)?, bincode::deserialize(&value)?);
        }
        all_accounts.extend(batch_context.write_set.clone());

        let new_batch_id = self.tip.batch_id + 1;
        let new_root = compute_state_commitment(&all_accounts, new_batch_id);
        
        let header = BlockHeader {
            batch_id: new_batch_id,
            prev_root: self.tip.new_root,
            new_root,
            tx_count: txs_to_process.len() as u32,
            open_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ..BlockHeader::genesis()
        };
        
        commit_batch(&self.storage, &header, &batch_context.write_set, &txs_to_process).await?;
        self.tip = header;
        Ok(())
    }
}

