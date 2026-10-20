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
    pub dialog: bool,
    pub release: Option<Arc<tokio::sync::Notify>>,
    pub denied: bool,
    pub timed_out: bool,
    pub delay: Duration,
}
impl Approval for FakeApproval {
    fn approve<'a>(
        &'a self,
        _request: &'a AcquireRequest,
        _pid: u32,
        cancel: Arc<AtomicBool>,
        work: teamy_broker::broker::work_guard::WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            let _dialog = self
                .dialog
                .then(|| work.phase(teamy_broker::broker::work_kind::WorkKind::Dialog));
            let _work = work;
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(release) = &self.release {
                release.notified().await;
            }
            tokio::time::sleep(self.delay).await;
            eyre::ensure!(
                !self.timed_out,
                teamy_broker::protocol::FailureKind::ApprovalTimedOut
            );
            Ok(!self.denied && !cancel.load(Ordering::SeqCst))
        })
    }
}
