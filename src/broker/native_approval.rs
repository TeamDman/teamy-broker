use super::approval::Approval;
use crate::protocol::AcquireRequest;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[derive(Debug)]
pub struct NativeApproval;
impl Approval for NativeApproval {
    fn fixture_decision<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: super::work_guard::WorkGuard,
        grant: Arc<super::credential_grant::CredentialGrant>,
    ) -> Pin<
        Box<
            dyn Future<Output = eyre::Result<super::approval_decision::ApprovalDecision>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            eyre::ensure!(grant.synthetic_fixture, "fixture grant required");
            super::credential_approvals::approve(request.clone(), pid, cancel, work, &grant)
        })
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
            if let Some(grant) = super::phone::Controller::resident()?.claim_permission(request)? {
                super::credential_approvals::approve(request.clone(), pid, cancel, work, &grant)
            } else {
                Ok(if self.approve(request, pid, cancel, work).await? {
                    super::approval_decision::ApprovalDecision::DesktopRead
                } else {
                    super::approval_decision::ApprovalDecision::Denied
                })
            }
        })
    }
    fn preview<'a>(
        &'a self,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: super::work_guard::WorkGuard,
    ) -> Pin<
        Box<dyn Future<Output = eyre::Result<crate::protocol::ApprovalPreviewResult>> + Send + 'a>,
    > {
        Box::pin(async move {
            let _pending_work = work.phase(super::work_kind::WorkKind::Approval);
            let shared =
                super::shared_preview::SharedPreviewHub::resident().begin(Arc::clone(&cancel))?;
            let request = Arc::clone(&shared);
            let mut result = tokio::task::spawn_blocking(move || {
                crate::platform::native_approval::preview_shared(pid, &cancel, work, request)
            })
            .await
            .map_err(|_redacted_error| {
                eyre::Report::new(crate::protocol::FailureKind::ApprovalUnavailable)
            })??;
            if result.disposition == crate::protocol::ApprovalPreviewDisposition::Unavailable {
                shared.resolve(result.disposition);
            }
            while shared.outcome().is_none() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            result.disposition = shared.outcome().expect("resolved preview");
            result.finished_unix_ms = crate::protocol::wire::unix_ms()?;
            Ok(result)
        })
    }
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: super::work_guard::WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        let request = request.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                crate::platform::native_approval::approve_tracked(
                    &request,
                    pid,
                    &cancel,
                    Some(work),
                )
            })
            .await
            .map_err(|_redacted_error| {
                eyre::Report::new(crate::protocol::FailureKind::ApprovalUnavailable)
            })?
        })
    }
}
