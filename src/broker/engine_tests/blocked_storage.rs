use crate::broker::lease::Lease;
use crate::broker::storage::Storage;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(super) struct BlockedStorage {
    pub started: Notify,
    released: Mutex<bool>,
    release: Condvar,
    pub published: Mutex<Vec<String>>,
    pub removed: Mutex<Vec<String>>,
    pub fail_remove: AtomicBool,
    pub block_remove: AtomicBool,
    pub remove_started: Notify,
    removal_released: Mutex<bool>,
    removal_release: Condvar,
}

impl BlockedStorage {
    pub fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }

    pub fn release_removal(&self) {
        *self.removal_released.lock().unwrap() = true;
        self.removal_release.notify_all();
    }
}

impl Storage for BlockedStorage {
    fn publish(&self, lease: &Lease) -> eyre::Result<()> {
        let released = self.released.lock().unwrap();
        self.started.notify_one();
        let (released, _) = self
            .release
            .wait_timeout_while(released, Duration::from_secs(5), |released| !*released)
            .unwrap();
        eyre::ensure!(*released, "synthetic publication wait expired");
        self.published.lock().unwrap().push(lease.id.clone());
        Ok(())
    }

    fn remove(&self, lease: &Lease) -> eyre::Result<()> {
        if self.block_remove.load(Ordering::SeqCst) {
            let released = self.removal_released.lock().unwrap();
            self.remove_started.notify_one();
            let (released, _) = self
                .removal_release
                .wait_timeout_while(released, Duration::from_secs(5), |released| !*released)
                .unwrap();
            eyre::ensure!(*released, "synthetic cleanup wait expired");
        }
        self.removed.lock().unwrap().push(lease.id.clone());
        eyre::ensure!(
            !self.fail_remove.load(Ordering::SeqCst),
            "synthetic cleanup failure"
        );
        Ok(())
    }
}
