use crate::types::{Account,Pubkey};
use std::collections::BTreeMap;

fn pseudo_poseidon_hash(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..32 { result[i] = a[i] ^ b[i]; }
    result
}

/// Computes the AccountsFoldHashV1 state commitment.
pub fn compute_state_commitment(
    accounts: &BTreeMap<Pubkey, Account>,
    batch_id: u64,
) -> [u8; 32] {
    let mut ds = [0u8; 32];
    ds[0..23].copy_from_slice(b"zelana:accounts-fold:v1");
    let mut batch_id_bytes = [0u8; 32];
    batch_id_bytes[..8].copy_from_slice(&batch_id.to_le_bytes());
    
    let mut current_state = pseudo_poseidon_hash(ds, batch_id_bytes);

    for (pubkey, account) in accounts {
        let mut balance_bytes = [0u8; 32];
        balance_bytes[..8].copy_from_slice(&account.balance.to_le_bytes());
        let mut nonce_bytes = [0u8; 32];
        nonce_bytes[..8].copy_from_slice(&account.nonce.to_le_bytes());
        
        let inner_hash = pseudo_poseidon_hash(balance_bytes, nonce_bytes);
        let leaf_hash = pseudo_poseidon_hash(pubkey.0, inner_hash);
        
        current_state = pseudo_poseidon_hash(current_state, leaf_hash);
    }
    
    let mut count_bytes = [0u8; 32];
    count_bytes[..8].copy_from_slice(&(accounts.len() as u64).to_le_bytes());
    
    pseudo_poseidon_hash(current_state, count_bytes)
}
