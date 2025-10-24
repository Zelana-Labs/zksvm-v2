use super::storage::Storage;
use crate::types::BlockHeader;
use chrono::Utc;
use rocksdb::IteratorMode;

pub async fn reconcile_databases_on_startup(storage: &Storage) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Running Startup Recovery Check ---");
    let cf_batches = storage.cf_batches();
    let latest_rocksdb_id = storage.rocksdb.iterator_cf(cf_batches, IteratorMode::End)
        .next()
        .and_then(|res| res.ok())
        .map(|(key, _)| u64::from_be_bytes(key.as_ref().try_into().unwrap()));

    let latest_sqlite_id: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM batches").fetch_one(&storage.sqlite).await.ok().flatten();
    let latest_rocksdb_id = latest_rocksdb_id.unwrap_or(0);
    let latest_sqlite_id = latest_sqlite_id.unwrap_or(0) as u64;

    println!("  - Latest batch in RocksDB: {}", latest_rocksdb_id);
    println!("  - Latest batch in SQLite:  {}", latest_sqlite_id);

    if latest_rocksdb_id > latest_sqlite_id {
        println!("  - Inconsistency detected! Reconciling SQLite...");
        for id in (latest_sqlite_id + 1)..=latest_rocksdb_id {
            if let Some(value) = storage.rocksdb.get_cf(cf_batches, id.to_be_bytes())? {
                let header = BlockHeader::from_bytes(value.as_slice().try_into()?)?;
                sqlx::query("INSERT OR REPLACE INTO batches (id, new_root, committed_at) VALUES (?, ?, ?)")
                    .bind(header.batch_id as i64)
                    .bind(&header.new_root.to_vec())
                    .bind(&Utc::now().to_rfc3339())
                    .execute(&storage.sqlite)
                    .await?;
                println!("    - Repaired batch {} in SQLite.", id);
            }
        }
    } else {
        println!("  - Databases are consistent.");
    }
    println!("--- Recovery Check Complete ---\n");
    Ok(())
}

