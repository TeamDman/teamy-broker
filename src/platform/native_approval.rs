use crate::protocol::AcquireRequest;
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
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: u32,
    wparam: usize,
    _lparam: isize,
    data: isize,
) -> i32 {
    // TDN_TIMER=4. Closing after 60 seconds is denial, never an approval.
    // SAFETY: data points to the live AtomicBool owned by the synchronous approve call.
    let cancelled = unsafe { &*(data as *const std::sync::atomic::AtomicBool) }
        .load(std::sync::atomic::Ordering::SeqCst);
    if notification == 4 && (wparam >= 60_000 || cancelled) {
        // SAFETY: callback receives the live task-dialog HWND; TDM_CLICK_BUTTON selects our deny button.
        unsafe { SendMessageW(hwnd, WM_USER + 102, DENY as usize, 0) };
    }
    0
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn approve(
    request: &AcquireRequest,
    pid: u32,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> eyre::Result<bool> {
    request.validate()?;
    let title = super::identity::wide("teamy-broker item approval");
    let instruction = super::identity::wide("Approve this exact credential use for 15 minutes?");
    let content = super::identity::wide(&format!(
        "Account: {}\nSecret reference: {}\nPurpose: {}\nRequesting app (claimed): {}\nConnected process ID: {}\nDestination: {}\nStorage: {}\n\nFixed 15-minute hard lease from approval. Activity never renews it. Purge cannot revoke copies or upstream tokens.\nThis cooperative same-user dialog can be spoofed. Closing or timing out denies.",
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
    ));
    let yes = super::identity::wide("Approve exact use");
    let no = super::identity::wide("Deny");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: APPROVE,
            pszButtonText: yes.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: DENY,
            pszButtonText: no.as_ptr(),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER,
        pszWindowTitle: title.as_ptr(),
        pszMainInstruction: instruction.as_ptr(),
        pszContent: content.as_ptr(),
        cButtons: 2,
        pButtons: buttons.as_ptr(),
        nDefaultButton: DENY,
        pfCallback: Some(callback),
        lpCallbackData: std::sync::Arc::as_ptr(cancel) as isize,
        ..Default::default()
    };
    let mut selected = DENY;
    // SAFETY: config and all its string/button pointers remain live for the synchronous native dialog call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            &raw mut selected,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    eyre::ensure!(result >= 0, "native approval unavailable");
    Ok(selected == APPROVE)
}
