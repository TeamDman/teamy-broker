use super::work_guard::WorkGuard;
use super::work_kind::WorkKind;
use super::work_snapshot::WorkSnapshot;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct WorkState(pub(crate) Mutex<WorkSnapshot>);
impl WorkState {
    pub(crate) fn request(self: &Arc<Self>) -> eyre::Result<WorkGuard> {
        let mut state = self.0.lock().expect("work state lock");
        eyre::ensure!(
            !state.stopping,
            crate::protocol::FailureKind::RequestCancelled
        );
        state.counts[WorkKind::Request as usize] += 1;
        Ok(WorkGuard {
            state: Arc::clone(self),
            kind: WorkKind::Request,
        })
    }
    pub(crate) fn phase(self: &Arc<Self>, kind: WorkKind) -> WorkGuard {
        self.0.lock().expect("work state lock").counts[kind as usize] += 1;
        WorkGuard {
            state: Arc::clone(self),
            kind,
        }
    }
    pub(crate) fn snapshot(&self) -> WorkSnapshot {
        self.0.lock().expect("work state lock").clone()
    }
    pub(crate) fn close(&self) {
        self.0.lock().expect("work state lock").stopping = true;
    }
    pub(crate) fn admit_stop(&self, force: bool, cached: bool, cleanup: bool) -> eyre::Result<()> {
        let mut state = self.0.lock().expect("work state lock");
        eyre::ensure!(
            force
                || (!state.stopping
                    && !cached
                    && !cleanup
                    && state.counts.iter().all(|count| *count == 0)),
            crate::protocol::FailureKind::ShutdownBlocked
        );
        state.stopping = true;
        Ok(())
    }
}
