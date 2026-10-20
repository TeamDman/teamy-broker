use super::pairing_prompt_context::PairingPromptContext;
use super::pairing_prompt_diagnostics::PairingPromptDiagnostics;
use super::pairing_prompt_state::PairingPromptState;
use crate::platform::approval_attention::DialogAttention;
use crate::platform::approval_presentation::DialogPresentation;
use crate::protocol::ApprovalPreviewEventKind;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::TASKDIALOG_BUTTON;
use windows_sys::Win32::UI::Controls::TASKDIALOGCONFIG;
use windows_sys::Win32::UI::Controls::TDF_ALLOW_DIALOG_CANCELLATION;
use windows_sys::Win32::UI::Controls::TDF_CALLBACK_TIMER;
use windows_sys::Win32::UI::Controls::TaskDialogIndirect;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_USER;
use zeroize::Zeroizing;
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: u32,
    wparam: usize,
    _lparam: isize,
    data: isize,
) -> i32 {
    // SAFETY: synchronous native dialog owns this live stack state throughout callbacks.
    let context = unsafe { &*(data as *const PairingPromptContext<'_>) };
    let state = context.state;
    if notification == 0 {
        #[cfg(test)]
        if let Some(a) = &state.confirmation {
            a.event("native_created");
        }
        super::preview_phase::emit(
            context.trace.as_deref(),
            super::preview_phase::PreviewPhase::DialogCreated,
        );
        // SAFETY: callback provides the live, same-process top-level native window.
        let protected = unsafe { SetWindowDisplayAffinity(hwnd, 0x11) != 0 };
        if let Some(diagnostics) = context.diagnostics {
            *diagnostics
                .protection
                .lock()
                .expect("synthetic protection lock") = Some(protected);
        }
        context.record(hwnd, ApprovalPreviewEventKind::Created, None);
    }
    if notification == 2 {
        if let Some(attempt) = &state.confirmation {
            let _ = attempt.native(wparam == 100);
        }
        if let Some(trace) = &context.trace {
            let live =
                !state.closed.load(Ordering::Acquire) && std::time::Instant::now() < state.until;
            let phase = if wparam == 100 {
                Some(super::preview_phase::PreviewPhase::LocalMatch)
            } else if live && (wparam == 101 || wparam == 2) {
                Some(super::preview_phase::PreviewPhase::LocalReject)
            } else {
                None
            };
            if let Some(phase) = phase {
                trace.emit(
                    phase,
                    super::preview_phase::PreviewMetadata {
                        accepted: Some(live && wparam == 100),
                        origin: Some(super::preview_phase::PreviewOrigin::Desktop),
                        ..Default::default()
                    },
                );
            }
        }
        context.record(
            hwnd,
            ApprovalPreviewEventKind::ButtonClicked,
            i32::try_from(wparam).ok(),
        );
    }
    if notification == 5 {
        #[cfg(test)]
        if let Some(a) = &state.confirmation {
            a.event("native_destroyed");
        }
        super::preview_phase::emit(
            context.trace.as_deref(),
            super::preview_phase::PreviewPhase::DialogDestroyed,
        );
        context.record(hwnd, ApprovalPreviewEventKind::Destroyed, None);
    }
    if notification == 4 && context.trace.as_ref().is_some_and(|trace| trace.failed()) {
        state.closed.store(true, Ordering::Release);
    }
    if notification == 4 && state.on_timer(|| context.present(hwnd)) {
        let phase = if std::time::Instant::now() >= state.until {
            super::preview_phase::PreviewPhase::LocalTimeout
        } else {
            super::preview_phase::PreviewPhase::LocalCancelled
        };
        super::preview_phase::emit(context.trace.as_deref(), phase);
        context.record(hwnd, ApprovalPreviewEventKind::DeadlineOrCancellation, None);
        // SAFETY: live dialog handle; IDCANCEL rejects this pairing attempt.
        unsafe { SendMessageW(hwnd, WM_USER + 102, 2, 0) };
    }
    0
}
pub(super) fn show(state: &PairingPromptState) -> eyre::Result<bool> {
    show_core(state, None, true)
}
pub(super) fn show_traced(
    state: &PairingPromptState,
    trace: std::sync::Arc<super::preview_phase::PreviewTrace>,
) -> eyre::Result<bool> {
    show_core_traced(state, None, true, Some(trace), false)
}
pub(super) fn show_core(
    state: &PairingPromptState,
    diagnostics: Option<&PairingPromptDiagnostics>,
    presentation_enabled: bool,
) -> eyre::Result<bool> {
    show_core_traced(state, diagnostics, presentation_enabled, None, false)
}
pub(super) fn show_persistent(state: &PairingPromptState) -> eyre::Result<bool> {
    show_core_traced(state, None, true, None, true)
}
fn show_core_traced(
    state: &PairingPromptState,
    diagnostics: Option<&PairingPromptDiagnostics>,
    presentation_enabled: bool,
    trace: Option<std::sync::Arc<super::preview_phase::PreviewTrace>>,
    persistent: bool,
) -> eyre::Result<bool> {
    let context = PairingPromptContext {
        state,
        presentation: DialogPresentation::default(),
        attention: DialogAttention::default(),
        diagnostics,
        presentation_enabled,
        trace,
        visible_reported: std::sync::atomic::AtomicBool::new(false),
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let title = wide(if diagnostics.is_some() {
        "TEST/DUMMY - teamy-broker pairing dialog probe"
    } else if persistent {
        "teamy-broker · pair phone for no-vault approvals"
    } else {
        "teamy-broker DUMMY ONLY · compare pairing code"
    });
    let raw_text = Zeroizing::new(format!(
        "Compare all four groups with the foreground phone app:\n\n{}\n\nPhone: {}\nTailscale node: {}\nEndpoint: {}:{}\n\nTap Match on BOTH screens only if every group matches. Reject a mismatch or an unexpected prompt. This authenticates this live UKEY2 session only, not a saved phone pairing. No credential request or value is sent before both confirmations. Dummy-only, no vault access. Pairing expires within 90 seconds; listener within 180 seconds.",
        *state.code,
        super::tailnet_peer::PHONE,
        super::tailnet_peer::PHONE_NODE,
        super::tailnet_peer::DESKTOP,
        super::tailnet_peer::PORT
    ));
    let raw_text = if persistent {
        Zeroizing::new(format!(
            "Compare all four groups on the phone:\n\n{}\n\nPERSISTENT NO-VAULT APPROVAL PAIRING\nDesktop {}:{} / {}\nPhone {} / {}\n\nMatch on BOTH screens only if every group agrees. This saves protected TLS device identities for future no-vault approval previews. It enables an exact Tailscale-only resident listener. Phone keys stay in Android Keystore; desktop keys use current-user DPAPI. No phone vault access or credential lease is enabled. Revoke with phone-revoke. Reject unexpected pairing. This comparison expires within the five-minute enrollment window.",
            *state.code,
            super::tailnet_peer::DESKTOP,
            super::tailnet_peer::PORT,
            super::tailnet_peer::DESKTOP_NODE,
            super::tailnet_peer::PHONE,
            super::tailnet_peer::PHONE_NODE
        ))
    } else {
        raw_text
    };
    let message_body = Zeroizing::new(wide(&raw_text));
    let yes = wide("Match · all groups agree");
    let no = wide("Reject / cancel pairing");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: 100,
            pszButtonText: yes.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 101,
            pszButtonText: no.as_ptr(),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER,
        pszWindowTitle: title.as_ptr(),
        pszContent: message_body.as_ptr(),
        cButtons: 2,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 101,
        pfCallback: Some(callback),
        lpCallbackData: std::ptr::from_ref(&context) as isize,
        ..Default::default()
    };
    let mut button = 0;
    // SAFETY: every pointer references live buffers/state for the synchronous call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            &raw mut button,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if let Some(diagnostics) = diagnostics {
        *diagnostics.hresult.lock().expect("synthetic result lock") = result;
        *diagnostics.button.lock().expect("synthetic button lock") = button;
    }
    if result < 0
        && let Some(attempt) = &state.confirmation
    {
        attempt.failure(crate::protocol::phone_status::PhoneFailureCategory::PairingUiUnavailable);
    }
    eyre::ensure!(result >= 0, "pairing UI unavailable");
    if let Some(attempt) = &state.confirmation {
        return Ok(attempt.accepted() && !state.closed.load(Ordering::Acquire));
    }
    Ok(button == 100
        && !state.closed.load(Ordering::Acquire)
        && std::time::Instant::now() < state.until)
}
