use crate::protocol::AcquireRequest;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
pub trait Approval: Send + Sync {
    fn preview<'a>(
        &'a self,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: super::work_guard::WorkGuard,
    ) -> Pin<
        Box<dyn Future<Output = eyre::Result<crate::protocol::ApprovalPreviewResult>> + Send + 'a>,
    > {
        Box::pin(async { Err(crate::protocol::FailureKind::ApprovalUnavailable.into()) })
    }
    fn decision<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: super::work_guard::WorkGuard,
    ) -> Pin<
        Box<
            dyn Future<Output = eyre::Result<super::approval_decision::ApprovalDecision>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(if self.approve(request, pid, cancel, work).await? {
                super::approval_decision::ApprovalDecision::DesktopRead
            } else {
                super::approval_decision::ApprovalDecision::Denied
            })
        })
    }
    fn fixture_decision<'a>(
        &'a self,
        _request: &'a AcquireRequest,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: super::work_guard::WorkGuard,
        _grant: Arc<super::credential_grant::CredentialGrant>,
    ) -> Pin<
        Box<
            dyn Future<Output = eyre::Result<super::approval_decision::ApprovalDecision>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { Err(crate::protocol::FailureKind::ApprovalUnavailable.into()) })
    }
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: super::work_guard::WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>>;
}
