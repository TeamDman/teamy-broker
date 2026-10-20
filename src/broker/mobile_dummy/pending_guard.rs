use std::sync::Arc;
// Drop cancels admission when a waiting future is timed out or disconnected.
pub(super) struct PendingGuard {
    pub queue: Arc<teamy_broker_handoff::queue::Queue>,
    pub id: String,
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.queue.cancel(&self.id);
    }
}
