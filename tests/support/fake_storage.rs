use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use teamy_broker::broker::lease::Lease;
use teamy_broker::broker::storage::Storage;
#[derive(Debug, Default)]
pub struct FakeStorage {
    pub published: Mutex<Vec<String>>,
    pub removals: AtomicUsize,
    pub fail_publish: AtomicBool,
    pub fail_remove: AtomicBool,
}
impl Storage for FakeStorage {
    fn publish(&self, lease: &Lease) -> eyre::Result<()> {
        eyre::ensure!(
            !self.fail_publish.load(Ordering::SeqCst),
            "synthetic cleanup registration failure"
        );
        self.published.lock().unwrap().push(lease.id.clone());
        Ok(())
    }
    fn remove(&self, _lease: &Lease) -> eyre::Result<()> {
        self.removals.fetch_add(1, Ordering::SeqCst);
        eyre::ensure!(
            !self.fail_remove.load(Ordering::SeqCst),
            "synthetic cleanup failure"
        );
        Ok(())
    }
}
