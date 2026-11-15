use super::Controller;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub(super) struct RuntimeGuard(Arc<Controller>);
impl RuntimeGuard {
    pub(super) fn new(controller: Arc<Controller>) -> Self {
        Self(controller)
    }
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        self.0.listener.store(false, Ordering::Release);
        self.0.connected.store(false, Ordering::Release);
        if self.0.runtime.load(Ordering::Acquire) != 3 {
            self.0.runtime.store(0, Ordering::Release);
        }
        self.0.actor_active.store(false, Ordering::Release);
    }
}
