use crate::broker::approval::Approval;
use crate::broker::work_guard::WorkGuard;
use crate::protocol::AcquireRequest;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use teamy_broker_handoff::queue::Queue;
/// Synthetic broadcast only. The existing production `NativeApproval` is unchanged.
#[derive(Debug)]
pub struct BroadcastDummyApproval {
    pub queue: Arc<Queue>,
}
impl Approval for BroadcastDummyApproval {
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        super::approval::approve_request(Arc::clone(&self.queue), request, pid, cancel, work, true)
    }
}
