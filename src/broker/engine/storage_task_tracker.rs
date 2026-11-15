use super::storage_task_guard::StorageTaskGuard;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(super) struct StorageTaskTracker {
    pub active: AtomicUsize,
    pub finished: Notify,
}

impl StorageTaskTracker {
    pub fn begin(self: &Arc<Self>) -> StorageTaskGuard {
        self.active.fetch_add(1, Ordering::SeqCst);
        StorageTaskGuard(Arc::clone(self))
    }

    pub async fn drain(&self) {
        loop {
            let finished = self.finished.notified();
            tokio::pin!(finished);
            finished.as_mut().enable();
            if self.active.load(Ordering::SeqCst) == 0 {
                return;
            }
            finished.await;
        }
    }
}
