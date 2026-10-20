use crate::protocol::AcquireRequest;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::ApprovalPreviewResult;
use crate::protocol::FailureKind;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
/// # Errors
/// Returns a typed failure if approval is unavailable, cancelled or expired.
pub fn approve(request: &AcquireRequest, pid: u32, cancel: &Arc<AtomicBool>) -> eyre::Result<bool> {
    approve_tracked(request, pid, cancel, None)
}
pub(crate) fn approve_tracked(
    request: &AcquireRequest,
    pid: u32,
    cancel: &Arc<AtomicBool>,
    work: Option<crate::broker::work_guard::WorkGuard>,
) -> eyre::Result<bool> {
    let result = credential_shared_inner(request, pid, cancel, work, None)?;
    eyre::ensure!(
        result.task_dialog_hresult >= 0,
        FailureKind::ApprovalUnavailable
    );
    match result.disposition {
        ApprovalPreviewDisposition::ApprovedTest => Ok(true),
        ApprovalPreviewDisposition::DeniedTest | ApprovalPreviewDisposition::Closed => Ok(false),
        ApprovalPreviewDisposition::Cancelled => Err(FailureKind::RequestCancelled.into()),
        ApprovalPreviewDisposition::TimedOut => Err(FailureKind::ApprovalTimedOut.into()),
        ApprovalPreviewDisposition::Unavailable => Err(FailureKind::ApprovalUnavailable.into()),
    }
}
pub(crate) fn credential_shared(
    request: &AcquireRequest,
    pid: u32,
    cancel: &Arc<AtomicBool>,
    work: Option<crate::broker::work_guard::WorkGuard>,
    shared: Arc<crate::broker::shared_preview::SharedPreview>,
) -> eyre::Result<ApprovalPreviewResult> {
    credential_shared_inner(request, pid, cancel, work, Some(shared))
}
fn credential_shared_inner(
    request: &AcquireRequest,
    pid: u32,
    cancel: &Arc<AtomicBool>,
    work: Option<crate::broker::work_guard::WorkGuard>,
    shared: Option<Arc<crate::broker::shared_preview::SharedPreview>>,
) -> eyre::Result<ApprovalPreviewResult> {
    request.validate()?;
    let fixture = shared
        .as_ref()
        .is_some_and(|request| request.synthetic_fixture());
    super::approval_dialog::show_shared(
        &super::approval_dialog_text::ApprovalDialogText {
            title: super::identity::wide("teamy-broker item approval"),
            instruction: super::identity::wide(if fixture {
                "ISOLATED TEST: enter only teamy-fixture-input-v1; no vault read"
            } else {
                "Choose how to provide this exact credential"
            }),
            details: super::identity::wide(&if fixture {
                format!(
                    "ISOLATED SYNTHETIC FIXTURE. No live provider, op process or vault read is possible. Enter only teamy-fixture-input-v1, or choose Use fixture provider for a built-in constant. Other entered values are rejected.\nAccount: {}\nReference: {}\nApp (claimed): {}\nPurpose: {}\nDestination: {}\nClient PID: {}\nMemory lease only. X/Esc closes this presentation; Deny cancels both devices.\n\nTest value (masked):\n\n\n",
                    request.account,
                    request.secret_ref,
                    request.requesting_app,
                    request.purpose,
                    request.destination,
                    pid
                )
            } else {
                format!(
                    "Account: {}\nSecret reference: {}\nPurpose: {}\nRequesting app (claimed): {}\nConnected process ID: {}\nDestination: {}\nStorage: {}\n\nFixed 15-minute hard lease from approval. Activity never renews it. Purge cannot revoke copies or upstream tokens.\nThis cooperative same-user dialog can be spoofed. With phone permission, X/Esc dismisses only this desktop window; Deny resolves both. Phone approval allows desktop provider read but cannot unlock desktop 1Password. Phone supply is explicitly user-operated. Timing out denies.\n\nCredential value (masked, explicit supply permission required):\n\n\n",
                    request.account,
                    request.secret_ref,
                    request.purpose,
                    request.requesting_app,
                    pid,
                    request.destination,
                    match &request.backend {
                        crate::protocol::Backend::Memory => "broker memory".to_string(),
                        crate::protocol::Backend::WindowsCredentialManager(s) =>
                            format!("Windows Credential Manager: {s}"),
                        crate::protocol::Backend::WellKnownFile(s) => format!("DPAPI file: {s}"),
                    }
                )
            }),
            yes: super::identity::wide(if fixture {
                "Use fixture provider"
            } else {
                "Use desktop 1Password"
            }),
            no: super::identity::wide("Deny"),
        },
        pid,
        cancel,
        work,
        false,
        shared,
    )
}
/// Fixed harmless presentation; neither this function nor its caller acquires a credential.
pub(crate) fn preview_shared(
    pid: u32,
    cancel: &Arc<AtomicBool>,
    work: crate::broker::work_guard::WorkGuard,
    shared: Arc<crate::broker::shared_preview::SharedPreview>,
) -> eyre::Result<ApprovalPreviewResult> {
    super::approval_dialog::show_shared(
        &super::approval_dialog_text::ApprovalDialogText {
            title: super::identity::wide("TEST/DUMMY - teamy-broker resident approval preview"),
            instruction: super::identity::wide(
                "TEST/DUMMY: can you see this resident broker window?",
            ),
            details: super::identity::wide(&format!(
                "NO VAULT OR CREDENTIAL REQUEST.\nConnected test client PID: {pid}\n\nChoose Deny test to confirm you saw this exact window. Approve test is also harmless.\nNo button can invoke 1Password, access storage/cache, or create a lease.\nThis request expires after 60 seconds or cancellation. Approve/Deny resolves every device. X/Esc only dismisses this desktop window; the phone may still decide.\nThis is the existing resident broker's native approval path, with fixed dummy content."
            )),
            yes: super::identity::wide("Approve test (no secret)"),
            no: super::identity::wide("Deny test"),
        },
        pid,
        cancel,
        Some(work),
        true,
        Some(shared),
    )
}
