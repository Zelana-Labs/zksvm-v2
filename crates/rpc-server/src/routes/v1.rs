use crate::{error::ApiError, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use rollup_core::types::{Account, BlockHeader, Pubkey, Signature, Transaction, TransactionType};
use rocksdb::{IteratorMode, OptimisticTransactionDB};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct TipResponse {
    batch_id: u64,
    new_root: String,
    flags: u32,
}

/// The expected JSON payload for the `send_transaction` endpoint.
/// Pubkeys and signatures are expected as hex-encoded strings.
#[derive(Deserialize)]
struct SendTxRequest {
    sender: String,
    recipient: String,
    tx_type: TransactionType,
    signature: String,
}

#[derive(Serialize)]
struct SendTxResponse {
    status: &'static str,
    signature: String,
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/tip", get(get_tip))
        .route("/accounts/{pubkey}", get(get_account))
        .route("/tx/{signature}", get(get_transaction))
        .route("/batches/{id}", get(get_batch))
        .route("/send_transaction", post(send_transaction))
}

async fn get_tip(State(state): State<AppState>) -> Result<Json<TipResponse>, ApiError> {
    let mut iter = state.storage.rocksdb.iterator_cf(state.storage.cf_batches(), IteratorMode::End);
    match iter.next() {
        Some(Ok((_, value))) => {
            let header = BlockHeader::from_bytes(value.as_ref().try_into().map_err(|_| ApiError::DatabaseUnavailable("Invalid header in DB".into()))?)
                .map_err(|_| ApiError::DatabaseUnavailable("Failed to deserialize header".into()))?;
            Ok(Json(TipResponse { batch_id: header.batch_id, new_root: hex::encode(header.new_root), flags: header.flags }))
        }
        _ => Err(ApiError::NotFound("Chain is empty; no tip found".to_string())),
    }
}

async fn get_account(State(state): State<AppState>, Path(pubkey_hex): Path<String>) -> Result<Json<Account>, ApiError> {
    println!("hi");
    if pubkey_hex.len() != 64 { return Err(ApiError::BadRequest("Public key must be a 64-character hex string.".into())); }
    println!("Fetching account: {}", pubkey_hex);
    let pubkey_bytes = hex::decode(pubkey_hex).map_err(|_| ApiError::BadRequest("Invalid hex characters in public key.".into()))?;
    match state.storage.rocksdb.get_cf(state.storage.cf_accounts(), &pubkey_bytes) {
        Ok(Some(bytes)) => Ok(Json(bincode::deserialize(&bytes).map_err(|_| ApiError::DatabaseUnavailable("Failed to deserialize account.".into()))?)),
        Ok(None) => Err(ApiError::NotFound("Account not found.".into())),
        Err(e) => Err(ApiError::DatabaseUnavailable(format!("DB error: {}", e))),
    }
}

async fn get_transaction(State(state): State<AppState>, Path(signature_hex): Path<String>) -> Result<Json<Transaction>, ApiError> {
    if signature_hex.len() != 64 { return Err(ApiError::BadRequest("Signature must be a 64-character hex string.".into())); }
    let sig_bytes = hex::decode(signature_hex).map_err(|_| ApiError::BadRequest("Invalid hex characters in signature.".into()))?;
    match state.storage.rocksdb.get_cf(state.storage.cf_txs(), &sig_bytes) {
        Ok(Some(bytes)) => Ok(Json(bincode::deserialize(&bytes).map_err(|_| ApiError::DatabaseUnavailable("Failed to deserialize transaction.".into()))?)),
        Ok(None) => Err(ApiError::NotFound("Transaction not found.".into())),
        Err(e) => Err(ApiError::DatabaseUnavailable(format!("DB error: {}", e))),
    }
}

async fn get_batch(State(state): State<AppState>, Path(id): Path<u64>) -> Result<Json<BlockHeader>, ApiError> {
    match state.storage.rocksdb.get_cf(state.storage.cf_batches(), id.to_be_bytes()) {
        Ok(Some(bytes)) => {
            let header = BlockHeader::from_bytes(bytes.as_slice().try_into().map_err(|_| ApiError::DatabaseUnavailable("Invalid header in DB".into()))?)
                .map_err(|_| ApiError::DatabaseUnavailable("Failed to deserialize header".into()))?;
            Ok(Json(header))
        }
        Ok(None) => Err(ApiError::NotFound(format!("Batch with ID {} not found.", id))),
        Err(e) => Err(ApiError::DatabaseUnavailable(format!("DB error: {}", e))),
    }
}

/// Receives a transaction, validates it, and forwards it to the Rollup Core's mempool.
async fn send_transaction(State(state):State<AppState>,Json(payload): Json<SendTxRequest>)->Result<Json<SendTxResponse>,ApiError>{
    println!("recieved");
// 1. Validate and decode hex-encoded fields.
    let sender_bytes = hex::decode(&payload.sender)
        .map_err(|_| ApiError::BadRequest("Invalid hex for sender pubkey.".to_string()))?;
    let recipient_bytes = hex::decode(&payload.recipient)
        .map_err(|_| ApiError::BadRequest("Invalid hex for recipient pubkey.".to_string()))?;
    let signature_bytes = hex::decode(&payload.signature)
        .map_err(|_| ApiError::BadRequest("Invalid hex for signature.".to_string()))?;

    //system level deposit from null address
    if sender_bytes == [0;32]{
        println!("[API] Detected system deposit for recipient: {}", payload.recipient);
        // This is a "mint" (deposit), not a "transfer".
        // We will update the database directly instead of sending to the sequencer.
        let amount = match payload.tx_type {
            TransactionType::Deposit { amount } => amount,
            _ => return Err(ApiError::BadRequest("System deposit must have tx_type 'Deposit'".into())),
        };

        // Get a reference to the DB and the account column family
        let db = &state.storage.rocksdb;
        let cf_accounts = state.storage.cf_accounts();
        let recipient_pubkey = Pubkey(recipient_bytes.try_into().map_err(|_| {
            ApiError::BadRequest("Recipient pubkey must be 32 bytes.".to_string())
        })?);
        // Fetch or create the account, update balance
        let mut updated_account: Account;

        match db.get_cf(cf_accounts, &recipient_pubkey.0) {
            Ok(Some(bytes)) => {
                let mut account: Account = bincode::deserialize(&bytes)
                    .map_err(|_| ApiError::DatabaseUnavailable("Failed to deserialize account.".into()))?;
                account.balance = account.balance.checked_add(amount)
                    .ok_or(ApiError::DatabaseUnavailable("Balance overflow".into()))?;
                account.nonce += 1;
                updated_account = account;
            }
            Ok(None) => {
                updated_account = Account {
                    balance: amount,
                    nonce: 1,
                };
            }
            Err(e) => return Err(ApiError::DatabaseUnavailable(format!("DB error: {}", e))),
        }

        // Save the updated account
        let updated_bytes = bincode::serialize(&updated_account)
            .map_err(|_| ApiError::DatabaseUnavailable("Failed to serialize account.".into()))?;
        db.put_cf(cf_accounts, &recipient_pubkey.0, updated_bytes).unwrap();

        println!(
            "[API] Successfully credited {} with {} zSOL",
            payload.recipient, amount
        );

        // Respond immediately — already processed.
        return Ok(Json(SendTxResponse {
            status: "processed",
            signature: payload.signature,
        }));
    }

    // 2. Construct the core Transaction type.
    let tx = Transaction {
        sender: Pubkey(sender_bytes.try_into().map_err(|_| {
            ApiError::BadRequest("Sender pubkey must be 32 bytes.".to_string())
        })?),
        recipient: Pubkey(recipient_bytes.try_into().map_err(|_| {
            ApiError::BadRequest("Recipient pubkey must be 32 bytes.".to_string())
        })?),
        tx_type: payload.tx_type,
        signature: Signature(signature_bytes.try_into().map_err(|_| {
            ApiError::BadRequest("Signature must be 32 bytes.".to_string())
        })?),
    };

    // 3. Send the transaction to the Rollup Core.
    state
        .tx_sender
        .send(tx)
        .await.map_err(|_| ApiError::DatabaseUnavailable("Sequencer channel is closed.".to_string()))?;

    // 4. Respond with acceptance.
    Ok(Json(SendTxResponse {
        status: "queued",
        signature: payload.signature,
    }))
}
