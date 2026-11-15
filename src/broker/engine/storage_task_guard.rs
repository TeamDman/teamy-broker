use super::storage_task_tracker::StorageTaskTracker;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub(super) struct StorageTaskGuard(pub Arc<StorageTaskTracker>);

impl Drop for StorageTaskGuard {
    fn drop(&mut self) {
        if self.0.active.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.finished.notify_waiters();
        }
    }
}
