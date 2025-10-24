use rocksdb::{DB, Options, ColumnFamilyDescriptor};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::{str::FromStr, sync::Arc};

pub struct Storage {
    pub rocksdb: Arc<DB>,
    pub sqlite: SqlitePool,
    cf_accounts: *const rocksdb::ColumnFamily,
    cf_txs: *const rocksdb::ColumnFamily,
    cf_batches: *const rocksdb::ColumnFamily,
    cf_tx_by_sender: *const rocksdb::ColumnFamily,
    cf_tx_by_time: *const rocksdb::ColumnFamily,
}

unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}

pub const CF_NAMES: &[&str] = &["accounts", "txs", "batches", "tx_by_sender", "tx_by_time"];

impl Storage {
    pub async fn new(rocksdb_path: &str, sqlite_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_atomic_flush(true);

        let cf_descriptors :Vec<_> = CF_NAMES.iter().map(|name| ColumnFamilyDescriptor::new(*name, Options::default())).collect();
        let db_arc = Arc::new(DB::open_cf_descriptors(&db_opts, rocksdb_path, cf_descriptors)?);

        let (cf_accounts, cf_txs, cf_batches, cf_tx_by_sender, cf_tx_by_time);
        { cf_accounts = db_arc.cf_handle("accounts").unwrap() as *const _; }
        { cf_txs = db_arc.cf_handle("txs").unwrap() as *const _; }
        { cf_batches = db_arc.cf_handle("batches").unwrap() as *const _; }
        { cf_tx_by_sender = db_arc.cf_handle("tx_by_sender").unwrap() as *const _; }
        { cf_tx_by_time = db_arc.cf_handle("tx_by_time").unwrap() as *const _; }

        let connect_options = SqliteConnectOptions::from_str(&format!("sqlite:{}", sqlite_path))?.create_if_missing(true);
        let pool = SqlitePoolOptions::new().connect_with(connect_options).await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS batches (
                id INTEGER PRIMARY KEY,
                new_root BLOB(32) NOT NULL,
                committed_at TEXT NOT NULL,
                proof_status TEXT DEFAULT 'Pending',
                l1_settlement_tx TEXT
            );",
        ).execute(&pool).await?;

        Ok(Self { rocksdb: db_arc, sqlite: pool, cf_accounts, cf_txs, cf_batches, cf_tx_by_sender, cf_tx_by_time })
    }

    #[inline] pub fn cf_accounts(&self) -> &rocksdb::ColumnFamily { unsafe { &*self.cf_accounts } }
    #[inline] pub fn cf_txs(&self) -> &rocksdb::ColumnFamily { unsafe { &*self.cf_txs } }
    #[inline] pub fn cf_batches(&self) -> &rocksdb::ColumnFamily { unsafe { &*self.cf_batches } }
    #[inline] pub fn cf_tx_by_sender(&self) -> &rocksdb::ColumnFamily { unsafe { &*self.cf_tx_by_sender } }
    #[inline] pub fn cf_tx_by_time(&self) -> &rocksdb::ColumnFamily { unsafe { &*self.cf_tx_by_time } }
}

