use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker::broker::approval::Approval;
use teamy_broker::protocol::AcquireRequest;
#[derive(Debug, Default)]
pub struct FakeApproval {
    pub calls: AtomicUsize,
    pub denied: bool,
    pub delay: Duration,
}
impl Approval for FakeApproval {
    fn approve<'a>(
        &'a self,
        _request: &'a AcquireRequest,
        _pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            Ok(!self.denied && !cancel.load(Ordering::SeqCst))
        })
    }
}
