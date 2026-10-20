use crate::broker::approval::Approval;
use crate::broker::approval_decision::ApprovalDecision;
use crate::broker::shared_preview::SharedPreview;
use crate::broker::work_guard::WorkGuard;
use crate::protocol::AcquireRequest;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[derive(Debug)]
pub struct SharedApproval(pub Arc<SharedPreview>);
impl Approval for SharedApproval {
    fn approve<'a>(
        &'a self,
        _request: &'a AcquireRequest,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: WorkGuard,
    ) -> std::pin::Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        Box::pin(async { Err(eyre::eyre!("shared path required")) })
    }
    fn decision<'a>(
        &'a self,
        _request: &'a AcquireRequest,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: WorkGuard,
    ) -> std::pin::Pin<Box<dyn Future<Output = eyre::Result<ApprovalDecision>> + Send + 'a>> {
        let shared = Arc::clone(&self.0);
        Box::pin(async move { Ok(ApprovalDecision::Shared(shared)) })
    }
}
