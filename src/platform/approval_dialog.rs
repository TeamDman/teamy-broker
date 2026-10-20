use super::approval_dialog_context::ApprovalDialogContext;
use super::approval_dialog_text::ApprovalDialogText;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::ApprovalPreviewEventKind;
use crate::protocol::ApprovalPreviewResult;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::TASKDIALOG_BUTTON;
use windows_sys::Win32::UI::Controls::TASKDIALOGCONFIG;
use windows_sys::Win32::UI::Controls::TDF_ALLOW_DIALOG_CANCELLATION;
use windows_sys::Win32::UI::Controls::TDF_CALLBACK_TIMER;
use windows_sys::Win32::UI::Controls::TaskDialogIndirect;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_USER;
pub(crate) const APPROVE: i32 = 100;
pub(crate) const DENY: i32 = 101;
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: u32,
    wparam: usize,
    _lparam: isize,
    data: isize,
) -> i32 {
    // SAFETY: context and all its borrowed state stay live during the synchronous native call.
    let context = unsafe { &*(data as *const ApprovalDialogContext<'_>) };
    if notification == 0 {
        context.state.created();
        if context.shared.as_ref().is_some_and(|s| s.allows_supply()) {
            *context.secret_input.lock().expect("input lock") =
                super::approval_secret_input::create(hwnd);
            if context.secret_input.lock().expect("input lock").is_none() {
                // SAFETY: disables only the owned task-dialog supply button.
                unsafe { SendMessageW(hwnd, WM_USER + 111, 102, 0) };
            }
        }
        context.record(hwnd, ApprovalPreviewEventKind::Created, None);
    }
    if notification == 2 {
        if let Some(shared) = &context.shared {
            match i32::try_from(wparam).ok() {
                Some(APPROVE) => {
                    if shared.is_credential() {
                        shared.request_provider();
                        // SAFETY: disables only this request's provider button; supply remains available.
                        unsafe { SendMessageW(hwnd, WM_USER + 111, APPROVE as usize, 0) };
                        return 1;
                    }
                    shared.resolve(ApprovalPreviewDisposition::ApprovedTest);
                }
                Some(102) => {
                    if let Some(handle) = *context.secret_input.lock().expect("input lock")
                        && super::approval_secret_input::read(handle)
                            .is_some_and(|bytes| shared.supply(bytes))
                    {
                        return 0;
                    }
                    return 1;
                }
                Some(DENY) => {
                    shared.resolve(ApprovalPreviewDisposition::DeniedTest);
                }
                _ => {} // X/Esc only dismiss this local presentation.
            }
        }
        if let Some(handle) = *context.secret_input.lock().expect("input lock") {
            super::approval_secret_input::clear(handle);
        }
        context.record(
            hwnd,
            ApprovalPreviewEventKind::ButtonClicked,
            i32::try_from(wparam).ok(),
        );
    }
    if notification == 5 {
        context.record(hwnd, ApprovalPreviewEventKind::Destroyed, None);
        context.secret_input.lock().expect("input lock").take();
        context.state.destroyed();
    }
    if notification == 4
        && let Some(shared) = &context.shared
        && shared.is_credential()
    {
        super::approval_dialog_feedback::update(hwnd, shared);
    }
    if notification == 4
        && (context
            .shared
            .as_ref()
            .is_some_and(|shared| shared.outcome().is_some())
            || context.state.on_timer(wparam, || {
                context.present_and_request_attention(hwnd);
            }))
    {
        context.record(hwnd, ApprovalPreviewEventKind::DeadlineOrCancellation, None);
        // SAFETY: callback supplies live task-dialog HWND; this selects only its Deny button.
        unsafe { SendMessageW(hwnd, WM_USER + 102, DENY as usize, 0) };
    }
    0
}
pub(crate) fn show_shared(
    text: &ApprovalDialogText,
    pid: u32,
    cancel: &Arc<AtomicBool>,
    work: Option<crate::broker::work_guard::WorkGuard>,
    preview: bool,
    shared: Option<Arc<crate::broker::shared_preview::SharedPreview>>,
) -> eyre::Result<ApprovalPreviewResult> {
    let started_unix_ms = crate::protocol::wire::unix_ms()?;
    let state = super::approval_dialog_state::ApprovalDialogState {
        work,
        dialog: std::sync::Mutex::new(None),
        cancel: Arc::clone(cancel),
        timed_out: AtomicBool::new(false),
        deadline: std::time::Instant::now() + std::time::Duration::from_mins(1),
    };
    let context = ApprovalDialogContext {
        state: &state,
        secret_input: std::sync::Mutex::new(None),
        shared,
        attention: super::approval_attention::DialogAttention::default(),
        presentation: super::approval_presentation::DialogPresentation::default(),
        presentation_outcome: std::sync::Mutex::new(None),
        events: preview.then(|| std::sync::Mutex::new(Vec::new())),
        outcome: std::sync::Mutex::new(None),
    };
    let supply_enabled = context.shared.as_ref().is_some_and(|s| s.allows_supply());
    let supply_label = super::identity::wide("Send entered credential");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: APPROVE,
            pszButtonText: text.yes.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: DENY,
            pszButtonText: text.no.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 102,
            pszButtonText: supply_label.as_ptr(),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER,
        pszWindowTitle: text.title.as_ptr(),
        pszMainInstruction: text.instruction.as_ptr(),
        pszContent: text.details.as_ptr(),
        cButtons: if supply_enabled { 3 } else { 2 },
        pButtons: buttons.as_ptr(),
        nDefaultButton: DENY,
        pfCallback: Some(callback),
        lpCallbackData: (&raw const context) as isize,
        ..Default::default()
    };
    let mut selected = DENY;
    // SAFETY: initialized config, context and all strings/buttons stay live throughout the synchronous native call.
    let hresult = unsafe {
        TaskDialogIndirect(
            &raw const config,
            &raw mut selected,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    finish(context, selected, hresult, started_unix_ms, pid, preview)
}
fn finish(
    context: ApprovalDialogContext<'_>,
    selected: i32,
    hresult: i32,
    started_unix_ms: u64,
    pid: u32,
    preview: bool,
) -> eyre::Result<ApprovalPreviewResult> {
    let validation = context.state.result(selected == APPROVE);
    let cancelled = validation.as_ref().is_err_and(|error| {
        error.downcast_ref::<crate::protocol::FailureKind>()
            == Some(&crate::protocol::FailureKind::RequestCancelled)
    });
    let disposition = disposition(
        cancelled,
        validation.is_err() && !cancelled,
        hresult,
        selected,
    );
    let (broker_executable, broker_sha256) = if preview {
        super::approval_preview_identity::executable()
    } else {
        (String::new(), None)
    };
    let events = context
        .events
        .map(|events| events.into_inner().expect("preview event lock"))
        .unwrap_or_default();
    // SAFETY: obtains this process's public numeric PID.
    let broker_pid = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() };
    let mut startup = windows_sys::Win32::System::Threading::STARTUPINFOW::default();
    // SAFETY: initialized writable structure receives only this process's startup metadata.
    unsafe { windows_sys::Win32::System::Threading::GetStartupInfoW(&raw mut startup) };
    Ok(ApprovalPreviewResult {
        disposition,
        started_unix_ms,
        finished_unix_ms: crate::protocol::wire::unix_ms()?,
        broker_pid,
        connected_client_pid: pid,
        broker_executable,
        broker_sha256,
        task_dialog_hresult: hresult,
        selected_button: selected,
        startup_flags: startup.dwFlags,
        startup_show_window: startup.wShowWindow,
        presentation: context
            .presentation_outcome
            .into_inner()
            .expect("preview presentation lock"),
        attention: context
            .outcome
            .into_inner()
            .expect("preview attention lock"),
        events,
    })
}
pub(crate) fn disposition(
    cancelled: bool,
    timed_out: bool,
    hresult: i32,
    selected: i32,
) -> ApprovalPreviewDisposition {
    if cancelled {
        ApprovalPreviewDisposition::Cancelled
    } else if timed_out {
        ApprovalPreviewDisposition::TimedOut
    } else if hresult < 0 {
        ApprovalPreviewDisposition::Unavailable
    } else if selected == APPROVE {
        ApprovalPreviewDisposition::ApprovedTest
    } else if selected == DENY {
        ApprovalPreviewDisposition::DeniedTest
    } else {
        ApprovalPreviewDisposition::Closed
    }
}
#[cfg(test)]
#[path = "approval_dialog_tests.rs"]
mod tests;
