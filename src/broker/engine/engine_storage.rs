use super::storage_task_tracker::StorageTaskTracker;
use crate::broker::lease::Lease;
use crate::broker::storage::Storage;
use crate::broker::work_kind::WorkKind;
use crate::broker::work_state::WorkState;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::task;
use zeroize::Zeroize;

/// Owns native effects and their cleanup until every tracked worker has finished.
pub(super) struct EngineStorage {
    pub backend: Arc<dyn Storage>,
    pub activity: Arc<WorkState>,
    pub leases: Arc<Mutex<HashMap<String, Lease>>>,
    pub pending: Mutex<Vec<Lease>>,
    pub failures: AtomicU64,
    pub epoch: AtomicU64,
    pub stopping: Arc<AtomicBool>,
    pub tasks: Arc<StorageTaskTracker>,
}

impl EngineStorage {
    pub fn new(backend: Arc<dyn Storage>, activity: Arc<WorkState>) -> Self {
        Self {
            backend,
            activity,
            leases: Arc::new(Mutex::new(HashMap::new())),
            pending: Mutex::new(Vec::new()),
            failures: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            stopping: Arc::new(AtomicBool::new(false)),
            tasks: Arc::default(),
        }
    }

    pub fn start_retirement(self: &Arc<Self>, leases: Vec<Lease>) -> oneshot::Receiver<()> {
        let (finished, completion) = oneshot::channel();
        if leases.is_empty() {
            let _ = finished.send(());
            return completion;
        }
        let state = Arc::clone(self);
        let tracked = self.tasks.begin();
        let work = self.activity.phase(WorkKind::Storage);
        task::spawn_blocking(move || {
            for lease in leases {
                state.remove(lease);
            }
            drop(work);
            drop(tracked);
            let _ = finished.send(());
        });
        completion
    }

    /// Runs only on an owned blocking worker. A failed cleanup retains no secret bytes.
    pub fn remove(&self, mut lease: Lease) {
        let result = catch_unwind(AssertUnwindSafe(|| self.backend.remove(&lease)));
        if !matches!(result, Ok(Ok(()))) {
            lease.secret.zeroize();
            self.failures.fetch_add(1, Ordering::SeqCst);
            self.pending.blocking_lock().push(lease);
        }
    }

    pub fn rollback(&self, id: &str) {
        let lease = self.leases.blocking_lock().remove(id);
        if let Some(lease) = lease {
            self.remove(lease);
        }
    }
}
