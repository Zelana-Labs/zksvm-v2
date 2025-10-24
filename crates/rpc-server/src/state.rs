use rollup_core::{db::Storage, types::Transaction};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    pub tx_sender : Sender<Transaction>
}

