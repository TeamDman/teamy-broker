use super::work_kind::WorkKind;
use super::work_state::WorkState;
use std::sync::Arc;

/// Keeps work visible through cancellation, blocking dialog teardown, and secret handoff.
#[derive(Debug)]
pub struct WorkGuard {
    pub(crate) state: Arc<WorkState>,
    pub(crate) kind: WorkKind,
}
impl WorkGuard {
    #[must_use]
    pub fn phase(&self, kind: WorkKind) -> Self {
        self.state.phase(kind)
    }
}
impl Drop for WorkGuard {
    fn drop(&mut self) {
        self.state.0.lock().expect("work state lock").counts[self.kind as usize] -= 1;
    }
}
