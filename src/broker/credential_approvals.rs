use super::shared_preview::SharedPreview;
use super::shared_preview::SharedPreviewHub;
use std::sync::Arc;
use std::sync::OnceLock;
pub fn hub() -> &'static SharedPreviewHub {
    static HUB: OnceLock<SharedPreviewHub> = OnceLock::new();
    HUB.get_or_init(SharedPreviewHub::default)
}
/// # Errors
/// Rejects invalid metadata, unavailable state, cancellation or expiry.
pub fn approve(
    request: crate::protocol::AcquireRequest,
    pid: u32,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    work: super::work_guard::WorkGuard,
    grant: &Arc<super::credential_grant::CredentialGrant>,
) -> eyre::Result<super::approval_decision::ApprovalDecision> {
    let shared = grant.begin(request.clone(), pid, Arc::clone(&cancel))?;
    let native = Arc::clone(&shared);
    tokio::task::spawn_blocking(
        move || match crate::platform::native_approval::credential_shared(
            &request,
            pid,
            &cancel,
            Some(work),
            Arc::clone(&native),
        ) {
            Ok(result) if result.task_dialog_hresult >= 0 => {}
            _ => {
                native.resolve(crate::protocol::ApprovalPreviewDisposition::Unavailable);
            }
        },
    );
    Ok(super::approval_decision::ApprovalDecision::Shared(shared))
}
/// # Errors
/// Rejects invalid metadata, unavailable state, cancellation or expiry.
pub async fn resolve(
    shared: Arc<SharedPreview>,
    provider: impl Future<Output = eyre::Result<zeroize::Zeroizing<Vec<u8>>>>,
) -> eyre::Result<zeroize::Zeroizing<Vec<u8>>> {
    let _guard = super::credential_resolution_guard::CredentialResolutionGuard(Arc::clone(&shared));
    let mut provider = std::pin::pin!(provider);
    let mut completed = false;
    loop {
        if shared.outcome().is_some() {
            return match decision(&shared)? {
                super::approval_decision::ApprovalDecision::Supplied(bytes) => Ok(bytes),
                super::approval_decision::ApprovalDecision::Denied => {
                    Err(crate::protocol::FailureKind::UserDenied.into())
                }
                _ => Err(crate::protocol::FailureKind::ApprovalUnavailable.into()),
            };
        }
        tokio::select! {
            result=&mut provider,if !completed&&shared.provider_requested.load(std::sync::atomic::Ordering::Acquire)=>{
                completed=true;
                match result{Ok(bytes) if !bytes.is_empty()=>{shared.provider_value(bytes);},_=>{shared.provider_failed.store(true,std::sync::atomic::Ordering::Release);}}
            },
            ()=tokio::time::sleep(std::time::Duration::from_millis(20))=>{},
        }
    }
}

/// # Errors
/// Rejects invalid metadata, unavailable state, cancellation or expiry.
pub fn decision(
    shared: &SharedPreview,
) -> eyre::Result<super::approval_decision::ApprovalDecision> {
    use super::approval_decision::ApprovalDecision as A;
    use crate::protocol::ApprovalPreviewDisposition as D;
    use crate::protocol::FailureKind as F;
    match shared.outcome() {
        Some(D::ApprovedTest) => Ok(shared.take_supply().map_or(A::DesktopRead, A::Supplied)),
        Some(D::DeniedTest | D::Closed) => Ok(A::Denied),
        Some(D::Cancelled) => Err(F::RequestCancelled.into()),
        Some(D::TimedOut) => Err(F::ApprovalTimedOut.into()),
        _ => Err(F::ApprovalUnavailable.into()),
    }
}
