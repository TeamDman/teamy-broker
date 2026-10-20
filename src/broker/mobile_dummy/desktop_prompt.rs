use super::desktop_prompt_state::DesktopPromptState;
use teamy_broker_handoff::local_decision::LocalDecision;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::TASKDIALOG_BUTTON;
use windows_sys::Win32::UI::Controls::TASKDIALOGCONFIG;
use windows_sys::Win32::UI::Controls::TDF_ALLOW_DIALOG_CANCELLATION;
use windows_sys::Win32::UI::Controls::TDF_CALLBACK_TIMER;
use windows_sys::Win32::UI::Controls::TaskDialogIndirect;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_USER;
const APPROVE: i32 = 100;
const DENY: i32 = 101;
const CANCEL: i32 = 102;
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: u32,
    wparam: usize,
    _lparam: isize,
    data: isize,
) -> i32 {
    // SAFETY: data references synchronous TaskDialogIndirect's live stack state.
    let state = unsafe { &*(data as *const DesktopPromptState) };
    if notification == 4
        && !state
            .queue
            .is_pending(&state.binding.request_id)
            .unwrap_or(false)
    {
        // SAFETY: timer receives a live dialog handle; IDCANCEL dismisses without resolving.
        unsafe { SendMessageW(hwnd, WM_USER + 102, 2, 0) };
    }
    if notification == 2 {
        let decision = match wparam {
            100 => Some(LocalDecision::SupplyDummy),
            101 => Some(LocalDecision::Deny),
            102 => Some(LocalDecision::Cancel),
            _ => None, // X/Escape/IDCANCEL dismiss only this prompt, leaving phone pending.
        };
        if let Some(decision) = decision {
            let _ = state.queue.resolve_desktop(&state.binding, decision);
        }
    }
    0
}
pub(super) fn show(state: &DesktopPromptState) -> eyre::Result<()> {
    let _dialog = state.work.phase(crate::broker::work_kind::WorkKind::Dialog);
    let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let title = wide("teamy-broker DUMMY ONLY broadcast approval");
    let content = wide(&format!(
        "Desktop: {}\nPhone: {}\nConsumer (claimed): {}\nConnected PID: {}\nAccount: {}\nExact field: {}\nPurpose: {}\nDestination: {}\nScope: {}\nLease: fixed 15 minutes; never renewed\nRequest deadline (Unix ms): {}\n\nEither this desktop or the phone can supply the fixed dummy value. First valid decision wins. Closing this window only dismisses this desktop prompt. Explicit Deny/Cancel resolves the shared request. Phone resolution or expiry closes this prompt. Purge cannot revoke consumer copies. No vault/1Password call occurs.",
        state.binding.desktop,
        state.binding.phone,
        state.binding.consumer,
        state.binding.consumer_pid,
        state.binding.account,
        state.binding.reference,
        state.binding.purpose,
        state.binding.destination,
        state.binding.scope,
        state.binding.expires_unix_ms
    ));
    let approve = wide("Supply dummy value & approve");
    let deny = wide("Deny shared request");
    let cancel = wide("Cancel shared request");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: APPROVE,
            pszButtonText: approve.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: DENY,
            pszButtonText: deny.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: CANCEL,
            pszButtonText: cancel.as_ptr(),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER,
        pszWindowTitle: title.as_ptr(),
        pszContent: content.as_ptr(),
        cButtons: u32::try_from(buttons.len())?,
        pButtons: buttons.as_ptr(),
        nDefaultButton: DENY,
        pfCallback: Some(callback),
        lpCallbackData: (std::ptr::from_ref(state)) as isize,
        ..Default::default()
    };
    // SAFETY: all configuration/string/button/state pointers live for this synchronous call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    eyre::ensure!(result >= 0, "dummy desktop UI unavailable");
    Ok(())
}
