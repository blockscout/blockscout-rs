use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::task::JoinHandle;

use super::CrosschainIndexerState;

/// Cleanup guard that ensures proper cleanup when an indexer task exits.
pub(crate) struct CleanupGuard {
    pub(crate) is_running: Arc<AtomicBool>,
    pub(crate) state: Arc<parking_lot::RwLock<CrosschainIndexerState>>,
    pub(crate) buffer_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    pub(crate) indexing_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    pub(crate) bridge_id: i32,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        tracing::debug!(
            bridge_id = self.bridge_id,
            "Indexer cleanup guard triggered"
        );

        self.is_running.store(false, Ordering::Release);

        if let Some(handle) = self.buffer_handle.write().take() {
            handle.abort();
        }

        let _ = self.indexing_handle.write().take();

        let current_state = self.state.read().clone();
        if !matches!(current_state, CrosschainIndexerState::Failed(_)) {
            *self.state.write() = CrosschainIndexerState::Idle;
        }
    }
}
