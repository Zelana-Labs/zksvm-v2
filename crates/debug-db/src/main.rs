use std::{env, path::PathBuf, sync::Arc};

use rocksdb::IteratorMode;
use rollup_core::{
    db::{Storage, CF_NAMES},
    types::{Account, Pubkey, Transaction},
};

use chrono::{DateTime, TimeZone, Utc};
use hex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let db_path = env::var("DB_PATH").unwrap_or_else(|_| "temp_db_for_demo".to_string());
    let rocks_path = PathBuf::from(&db_path).join("rocksdb");
    let sqlite_path = PathBuf::from(&db_path).join("checkpoints.db");

    let storage = Arc::new(
        Storage::new(
            rocks_path.to_str().unwrap(),
            sqlite_path.to_str().unwrap(),
        )
        .await?,
    );

    println!("📦 RocksDB path: {}\n", rocks_path.display());
    println!("Available Column Families:");
    for name in CF_NAMES {
        println!("  - {}", name);
    }
    println!("\n──────────────────────────────\n");

    for &cf_name in CF_NAMES {
        println!("🔍 Inspecting Column Family: {}\n", cf_name);

        let cf = match cf_name {
            "accounts" => storage.cf_accounts(),
            "txs" => storage.cf_txs(),
            "batches" => storage.cf_batches(),
            "tx_by_sender" => storage.cf_tx_by_sender(),
            "tx_by_time" => storage.cf_tx_by_time(),
            _ => continue,
        };

        let mut count = 0usize;

        for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
            let (key_bytes, value_bytes) = entry?;
            count += 1;

            match cf_name {
                // ---------------- ACCOUNTS ----------------
                "accounts" => {
                    let pubkey: Pubkey = bincode::deserialize(&key_bytes)?;
                    let account: Account = bincode::deserialize(&value_bytes)?;
                    println!(
                        "👤 Account: {}\n   💰 Balance: {}\n   🔄 Nonce: {}\n",
                        hex::encode(pubkey.0),
                        account.balance,
                        account.nonce
                    );
                }

                // ---------------- TXS ----------------
                "txs" => {
                    let tx_id: Pubkey = bincode::deserialize(&key_bytes)?;
                    let tx: Transaction = bincode::deserialize(&value_bytes)?;

                    println!(
                        "🧾 Tx Key: {}\n   📤 Sender: {}\n   📥 Recipient: {}\n   💸 Type: {:?}\n",
                        hex::encode(tx_id.0),
                        hex::encode(tx.sender.0),
                        hex::encode(tx.recipient.0),
                        tx.tx_type
                    );
                }

                // ---------------- BATCHES ----------------
                //
                // We don't rely on BlockHeader/serde here.
                // We parse the known layout directly:
                // key:   u64 (batch_id, big endian)
                // value: magic[4] | hdr_version[2] | batch_id[8] | prev_root[32] | new_root[32] | ...rest
                //
                "batches" => {
                    if key_bytes.len() >= 8 && value_bytes.len() >= 4 + 2 + 8 + 32 + 32 {
                        let batch_id =
                            u64::from_be_bytes(key_bytes[..8].try_into().unwrap());

                        let magic = &value_bytes[0..4];
                        let hdr_version =
                            u16::from_be_bytes(value_bytes[4..6].try_into().unwrap());
                        let _batch_id_in_value =
                            u64::from_be_bytes(value_bytes[6..14].try_into().unwrap());
                        let _prev_root = &value_bytes[14..46];
                        let new_root = &value_bytes[46..78];

                        println!("🪣 Batch #{}", batch_id);
                        println!("   🔣 Magic: {}", hex::encode(magic));
                        println!("   📦 Version: {}", hdr_version);
                        println!("   🌳 New Root: {}\n", hex::encode(new_root));
                    } else {
                        println!(
                            "⚠️ Invalid batch entry: key={} value={}\n",
                            hex::encode(&key_bytes),
                            hex::encode(&value_bytes)
                        );
                    }
                }

                // ---------------- TX_BY_SENDER ----------------
                //
                // key = sender_pubkey[32] || ts[8] || maybe tx_id[..]
                // value often empty (index only)
                //
                "tx_by_sender" => {
                    if key_bytes.len() >= 32 + 8 {
                        let sender = &key_bytes[..32];
                        let ts_bytes = &key_bytes[32..40];
                        let ts = u64::from_be_bytes(ts_bytes.try_into().unwrap());

                        let time: DateTime<Utc> =
                            Utc.timestamp_opt(ts as i64, 0).single().unwrap_or_else(|| Utc.timestamp(0, 0));

                        // Optional: rest of key might be tx_id, print if present
                        let extra = if key_bytes.len() > 40 {
                            format!("   🔑 Extra (tx id?): {}\n", hex::encode(&key_bytes[40..]))
                        } else {
                            "".to_string()
                        };

                        print!(
                            "📤 Sender: {}\n   🕒 Timestamp: {} UTC\n{}",
                            hex::encode(sender),
                            time,
                            extra
                        );
                    } else {
                        println!(
                            "⚠️ Invalid tx_by_sender key: {}\n",
                            hex::encode(&key_bytes)
                        );
                    }
                }

                // ---------------- TX_BY_TIME ----------------
                //
                // key = ts[8] || maybe tx_id[..]
                // value often empty
                //
                "tx_by_time" => {
                    if key_bytes.len() >= 8 {
                        let ts_bytes = &key_bytes[..8];
                        let ts = u64::from_be_bytes(ts_bytes.try_into().unwrap());

                        let time: DateTime<Utc> =
                            Utc.timestamp_opt(ts as i64, 0).single().unwrap_or_else(|| Utc.timestamp(0, 0));

                        let extra = if key_bytes.len() > 8 {
                            format!("   🔑 Extra (tx id?): {}\n", hex::encode(&key_bytes[8..]))
                        } else {
                            "".to_string()
                        };

                        print!("🕒 Timestamp: {} UTC\n{}\n", time, extra);
                    } else {
                        println!(
                            "⚠️ Invalid tx_by_time key: {}\n",
                            hex::encode(&key_bytes)
                        );
                    }
                }

                // Fallback (shouldn't hit for known CFs)
                _ => {
                    println!(
                        "Key: {}\nValue: {}\n",
                        hex::encode(&key_bytes),
                        hex::encode(&value_bytes)
                    );
                }
            }
        }

        if count == 0 {
            println!("(empty)\n");
        }

        println!("──────────────────────────────\n");
    }

    Ok(())
}
