use super::pairing_prompt_context::PairingPromptContext;
use super::pairing_prompt_state::PairingPromptState;
use crate::platform::approval_attention::DialogAttention;
use crate::platform::approval_presentation::DialogPresentation;
use crate::protocol::phone_status::PhoneFailureCategory;
use std::mem;
use std::ptr;
use std::sync::atomic::Ordering;
use std::time::Instant;
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
        if let Some(attempt) = &state.confirmation {
            attempt.event("native_created");
        }
        // SAFETY: callback provides the live, same-process top-level native window.
        let _ = unsafe { SetWindowDisplayAffinity(hwnd, 0x11) };
    }
    if notification == 2
        && let Some(attempt) = &state.confirmation
    {
        let _ = attempt.native(wparam == 100);
    }
    #[cfg(test)]
    if notification == 5
        && let Some(attempt) = &state.confirmation
    {
        attempt.event("native_destroyed");
    }
    if notification == 4 && state.on_timer(|| context.present(hwnd)) {
        // SAFETY: live dialog handle; IDCANCEL rejects this pairing attempt.
        unsafe { SendMessageW(hwnd, WM_USER + 102, 2, 0) };
    }
    0
}
pub(super) fn show_persistent(state: &PairingPromptState) -> eyre::Result<bool> {
    let context = PairingPromptContext {
        state,
        presentation: DialogPresentation::default(),
        attention: DialogAttention::default(),
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let title = wide("teamy-broker · pair phone for no-vault approvals");
    let raw_text = Zeroizing::new(format!(
        "Compare all four groups on the phone:\n\n{}\n\nPERSISTENT NO-VAULT APPROVAL PAIRING\n\nMatch on BOTH screens only if every group agrees. This saves protected TLS device identities for future no-vault approval previews. It enables an exact Tailscale-only resident listener. Phone keys stay in Android Keystore; desktop keys use current-user DPAPI. No phone vault access or credential lease is enabled. Revoke with phone-revoke. Reject unexpected pairing. This comparison expires within the five-minute enrollment window.",
        *state.code
    ));
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
        cbSize: u32::try_from(mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER,
        pszWindowTitle: title.as_ptr(),
        pszContent: message_body.as_ptr(),
        cButtons: 2,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 101,
        pfCallback: Some(callback),
        lpCallbackData: ptr::from_ref(&context) as isize,
        ..Default::default()
    };
    let mut button = 0;
    // SAFETY: every pointer references live buffers/state for the synchronous call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            &raw mut button,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if result < 0
        && let Some(attempt) = &state.confirmation
    {
        attempt.failure(PhoneFailureCategory::PairingUiUnavailable);
    }
    eyre::ensure!(result >= 0, "pairing UI unavailable");
    if let Some(attempt) = &state.confirmation {
        return Ok(attempt.accepted() && !state.closed.load(Ordering::Acquire));
    }
    Ok(button == 100 && !state.closed.load(Ordering::Acquire) && Instant::now() < state.until)
}
