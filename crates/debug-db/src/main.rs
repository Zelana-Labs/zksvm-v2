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

    println!("\n╔════════════════════════════════════════════════════════════════════╗");
    println!("║                      ROCKSDB DATABASE INSPECTOR                    ║");
    println!("║ Path: {:<58} ║", truncate(&rocks_path.display().to_string(), 58));
    println!("╚════════════════════════════════════════════════════════════════════╝");

    for &cf_name in CF_NAMES {
        match cf_name {
            // ========== ACCOUNTS ==========
            "accounts" => {
                let cf = storage.cf_accounts();
                let mut rows: Vec<Vec<String>> = Vec::new();

                for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
                    let (key_bytes, value_bytes) = entry?;
                    let pubkey: Pubkey = bincode::deserialize(&key_bytes)?;
                    let account: Account = bincode::deserialize(&value_bytes)?;

                    rows.push(vec![
                        hex::encode(pubkey.0),
                        account.balance.to_string(),
                        account.nonce.to_string(),
                    ]);
                }

                print_table_header("ACCOUNTS", rows.len());
                if rows.is_empty() {
                    print_empty_table();
                } else {
                    print_wrapped_table(
                        &["Pubkey", "Balance", "Nonce"],
                        &[44, 11, 5],
                        &["<", ">", ">"],
                        &rows,
                    );
                }
            }

            // ========== TXS ==========
            "txs" => {
                let cf = storage.cf_txs();
                let mut rows: Vec<Vec<String>> = Vec::new();

                for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
                    let (key_bytes, value_bytes) = entry?;
                    let tx_id: Pubkey = bincode::deserialize(&key_bytes)?;
                    let tx: Transaction = bincode::deserialize(&value_bytes)?;

                    rows.push(vec![
                        hex::encode(tx_id.0),
                        hex::encode(tx.sender.0),
                        hex::encode(tx.recipient.0),
                        format!("{:?}", tx.tx_type),
                    ]);
                }

                print_table_header("TRANSACTIONS", rows.len());
                if rows.is_empty() {
                    print_empty_table();
                } else {
                    print_wrapped_table(
                        &["Transaction ID", "Sender", "Recipient", "Type"],
                        &[44, 44, 44, 20],
                        &["<", "<", "<", "<"],
                        &rows,
                    );
                }
            }

            // ========== BATCHES ==========
            "batches" => {
                let cf = storage.cf_batches();
                let mut rows: Vec<Vec<String>> = Vec::new();

                for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
                    let (key_bytes, value_bytes) = entry?;

                    if key_bytes.len() >= 8 && value_bytes.len() >= 4 + 2 + 8 + 32 + 32 {
                        let batch_id = u64::from_be_bytes(key_bytes[..8].try_into().unwrap());
                        let magic = &value_bytes[0..4];
                        let hdr_version =
                            u16::from_be_bytes(value_bytes[4..6].try_into().unwrap());
                        let new_root = &value_bytes[46..78];

                        rows.push(vec![
                            batch_id.to_string(),
                            hex::encode(magic),
                            hdr_version.to_string(),
                            hex::encode(new_root),
                        ]);
                    }
                }

                print_table_header("BATCHES", rows.len());
                if rows.is_empty() {
                    print_empty_table();
                } else {
                    print_wrapped_table(
                        &["Batch", "Magic", "Version", "New Root"],
                        &[6, 8, 7, 44],
                        &[">", "<", ">", "<"],
                        &rows,
                    );
                }
            }

            // ========== TX_BY_SENDER ==========
            "tx_by_sender" => {
                let cf = storage.cf_tx_by_sender();
                let mut rows: Vec<Vec<String>> = Vec::new();

                for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
                    let (key_bytes, _value_bytes) = entry?;

                    if key_bytes.len() >= 32 + 8 {
                        let sender = &key_bytes[..32];
                        let ts_bytes = &key_bytes[32..40];
                        let ts = u64::from_be_bytes(ts_bytes.try_into().unwrap());
                        let time = decode_timestamp_nanos(ts);

                        let extra = if key_bytes.len() > 40 {
                            hex::encode(&key_bytes[40..])
                        } else {
                            String::new()
                        };

                        rows.push(vec![
                            hex::encode(sender),
                            time.to_rfc3339(),
                            extra,
                        ]);
                    }
                }

                print_table_header("TXS BY SENDER", rows.len());
                if rows.is_empty() {
                    print_empty_table();
                } else {
                    print_wrapped_table(
                        &["Sender", "Timestamp", "Extra"],
                        &[44, 25, 44],
                        &["<", "<", "<"],
                        &rows,
                    );
                }
            }

            // ========== TX_BY_TIME ==========
            "tx_by_time" => {
                let cf = storage.cf_tx_by_time();
                let mut rows: Vec<Vec<String>> = Vec::new();

                for entry in storage.rocksdb.iterator_cf(&cf, IteratorMode::Start) {
                    let (key_bytes, _value_bytes) = entry?;

                    if key_bytes.len() >= 8 {
                        let ts_bytes = &key_bytes[..8];
                        let ts = u64::from_be_bytes(ts_bytes.try_into().unwrap());
                        let time = decode_timestamp_nanos(ts);

                        let extra = if key_bytes.len() > 8 {
                            hex::encode(&key_bytes[8..])
                        } else {
                            String::new()
                        };

                        rows.push(vec![
                            time.to_rfc3339(),
                            extra,
                        ]);
                    }
                }

                print_table_header("TXS BY TIME", rows.len());
                if rows.is_empty() {
                    print_empty_table();
                } else {
                    print_wrapped_table(
                        &["Timestamp", "Extra"],
                        &[25, 44],
                        &["<", "<"],
                        &rows,
                    );
                }
            }

            _ => {
                print_table_header(&cf_name.to_uppercase(), 0);
                println!("╔════════════════════════════════════════════════════════════════════╗");
                println!("║                    (Unsupported Column Family)                     ║");
                println!("╚════════════════════════════════════════════════════════════════════╝");
            }
        }
    }

    println!("\n╔════════════════════════════════════════════════════════════════════╗");
    println!("║                     INSPECTION COMPLETE                            ║");
    println!("╚════════════════════════════════════════════════════════════════════╝\n");

    Ok(())
}

fn print_table_header(name: &str, count: usize) {
    println!("┌────────────────────────────────────────────────────────────────────┐");
    println!("│ 📊 {:^62} │", format!("{} ({})", name, count));
    println!("└────────────────────────────────────────────────────────────────────┘");
}

fn print_empty_table() {
    println!("╔════════════════════════════════════════════════════════════════════╗");
    println!("║                            (No data)                               ║");
    println!("╚════════════════════════════════════════════════════════════════════╝");
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - (max_len - 3)..])
    }
}

/// Interpret `ts` as nanoseconds since Unix epoch and convert to `DateTime<Utc>`.
/// Valt terug op 1970-01-01T00:00:00Z als het ongeldig is.
fn decode_timestamp_nanos(ts: u64) -> DateTime<Utc> {
    let secs = (ts / 1_000_000_000) as i64;
    let nsecs = (ts % 1_000_000_000) as u32;

    Utc.timestamp_opt(secs, nsecs)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap())
}

/// Generic table printer with wrapping.
fn print_wrapped_table(
    headers: &[&str],
    widths: &[usize],
    aligns: &[&str],
    rows: &[Vec<String>],
) {
    assert_eq!(headers.len(), widths.len());
    assert_eq!(widths.len(), aligns.len());

    // top border
    print!("╔");
    for (i, w) in widths.iter().enumerate() {
        print!("{}", "═".repeat(w + 2));
        if i < widths.len() - 1 {
            print!("╦");
        }
    }
    println!("╗");

    // header row
    print!("║");
    for ((h, w), _) in headers.iter().zip(widths.iter()).zip(aligns.iter()) {
        let formatted = format!("{:^width$}", h, width = *w);
        print!(" {} ║", formatted);
    }
    println!();

    // header separator
    print!("╠");
    for (i, w) in widths.iter().enumerate() {
        print!("{}", "═".repeat(w + 2));
        if i < widths.len() - 1 {
            print!("╬");
        }
    }
    println!("╣");

    // data rows
    for (row_idx, row) in rows.iter().enumerate() {
        let is_last = row_idx == rows.len() - 1;
        print_wrapped_row(row, widths, aligns, is_last);
    }

    // bottom border
    if !rows.is_empty() {
        print!("╚");
        for (i, w) in widths.iter().enumerate() {
            print!("{}", "═".repeat(w + 2));
            if i < widths.len() - 1 {
                print!("╩");
            }
        }
        println!("╝");
    }
}

/// Wrap a single cell string into multiple lines of at most `width` chars.
fn wrap_cell(s: &str, width: usize) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let end = (i + width).min(s.len());
        out.push(s[i..end].to_string());
        i = end;
    }
    out
}

/// Print one logical table row, wrapping long cells onto multiple lines.
fn print_wrapped_row(cells: &[String], widths: &[usize], aligns: &[&str], is_last: bool) {
    let wrapped: Vec<Vec<String>> = cells
        .iter()
        .zip(widths.iter())
        .map(|(s, &w)| wrap_cell(s, w))
        .collect();

    let max_lines = wrapped.iter().map(|lines| lines.len()).max().unwrap_or(0);

    for line_idx in 0..max_lines {
        print!("║");
        for (col_idx, col_lines) in wrapped.iter().enumerate() {
            let w = widths[col_idx];
            let content = col_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");

            let formatted = match aligns[col_idx] {
                ">" => format!("{:>width$}", content, width = w),
                "^" => format!("{:^width$}", content, width = w),
                _ => format!("{:<width$}", content, width = w),
            };

            print!(" {} ║", formatted);
        }
        println!();
    }

    if !is_last {
        print!("╟");
        for (i, &w) in widths.iter().enumerate() {
            print!("{}", "─".repeat(w + 2));
            if i < widths.len() - 1 {
                print!("╫");
            }
        }
        println!("╢");
    }
}
