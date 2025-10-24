use crate::db::Storage;
use crate::types::{Account, Pubkey, Transaction, TransactionType};
use std::collections::HashMap;

pub struct BatchContext<'a> {
    pub write_set: HashMap<Pubkey, Account>,
    storage: &'a Storage,
}

impl<'a> BatchContext<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { write_set: HashMap::new(), storage }
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.write_set.get(pubkey).cloned().or_else(|| {
            self.storage.rocksdb.get_cf(self.storage.cf_accounts(), &pubkey.0)
                .ok()
                .flatten()
                .and_then(|bytes| bincode::deserialize(&bytes).ok())
        })
    }
    
    pub fn execute_transaction(&mut self, tx: &Transaction) -> Result<(), String> {
        match tx.tx_type {
            TransactionType::Transfer { amount } => self.execute_transfer(tx, amount),
            TransactionType::Deposit { amount } => self.execute_deposit(tx, amount),
        }
    }

    fn execute_transfer(&mut self, tx: &Transaction, amount: u64) -> Result<(), String> {
        let mut sender = self.get_account(&tx.sender).ok_or("Sender not found")?;
        let mut recipient = self.get_account(&tx.recipient).unwrap_or(Account { balance: 0, nonce: 0 });

        if sender.balance < amount { return Err("Insufficient funds".to_string()); }
        
        sender.balance -= amount;
        sender.nonce += 1;
        recipient.balance += amount;

        self.write_set.insert(tx.sender, sender);
        self.write_set.insert(tx.recipient, recipient);
        Ok(())
    }
    
    fn execute_deposit(&mut self, tx: &Transaction, amount: u64) -> Result<(), String> {
        let mut recipient = self.get_account(&tx.recipient).unwrap_or(Account { balance: 0, nonce: 0 });
        recipient.balance += amount;
        self.write_set.insert(tx.recipient, recipient);
        Ok(())
    }
}

