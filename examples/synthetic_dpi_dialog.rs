//! Standalone non-credential `TaskDialog` DPI smoke test. No broker or backend access.
#[cfg(windows)]
use windows_sys::Win32::Foundation::HWND;

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetThreadDpiAwarenessContext() -> *mut std::ffi::c_void;
    fn AreDpiAwarenessContextsEqual(
        first: *mut std::ffi::c_void,
        second: *mut std::ffi::c_void,
    ) -> i32;
    fn GetDpiForWindow(window: HWND) -> u32;
}

#[cfg(windows)]
unsafe extern "system" fn callback(
    window: HWND,
    notification: u32,
    elapsed: usize,
    _parameter: isize,
    data: isize,
) -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE;
    use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOSIZE;
    use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER;
    use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos;
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_USER;

    let state = data as *mut [i32; 4];
    if notification == 0 {
        // SAFETY: copy the live caller-owned coordinates before calling potentially reentrant window APIs.
        let position = unsafe { *state };
        // SAFETY: the notification supplies the live synthetic window; only this test dialog is positioned.
        unsafe {
            SetWindowPos(
                window,
                std::ptr::null_mut(),
                position[0],
                position[1],
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    } else if notification == 4 && elapsed >= 3_000 {
        // SAFETY: query the synthetic dialog after its per-monitor transition has settled.
        let observed = i32::try_from(unsafe { GetDpiForWindow(window) }).unwrap_or(0);
        // SAFETY: callback state remains live; no state reference is held across reentrant window calls.
        unsafe { (*state)[3] = observed };
        // SAFETY: the live synthetic HWND receives TDM_CLICK_BUTTON for its standard Close button.
        unsafe { SendMessageW(window, WM_USER + 102, 8, 0) };
    }
    0
}

#[cfg(windows)]
fn main() -> eyre::Result<()> {
    use std::io::Write;
    use windows_sys::Win32::UI::Controls::TASKDIALOGCONFIG;
    use windows_sys::Win32::UI::Controls::TDCBF_CLOSE_BUTTON;
    use windows_sys::Win32::UI::Controls::TDF_CALLBACK_TIMER;
    use windows_sys::Win32::UI::Controls::TaskDialogIndirect;
    let args = std::env::args()
        .skip(1)
        .map(|arg| arg.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()?;
    eyre::ensure!(
        args.len() == 3,
        "usage: synthetic_dpi_dialog X Y EXPECTED_DPI"
    );
    let mut state = [args[0], args[1], args[2], 0];
    // SAFETY: only inspects this thread's awareness context.
    let awareness = unsafe { GetThreadDpiAwarenessContext() };
    // SAFETY: -4 is the documented DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 pseudo-handle.
    let native =
        unsafe { AreDpiAwarenessContextsEqual(awareness, -4_isize as *mut std::ffi::c_void) };
    eyre::ensure!(native != 0, "candidate is not PerMonitorV2 aware");
    let title: Vec<_> = "teamy-broker synthetic DPI test\0".encode_utf16().collect();
    let instruction: Vec<_> = "Synthetic dialog: no credential access\0"
        .encode_utf16()
        .collect();
    let content: Vec<_> = "Native Common Controls v6 text at the existing monitor scale.\nThis standalone test never contacts the broker.\nCloses automatically after three seconds.\0".encode_utf16().collect();
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwFlags: TDF_CALLBACK_TIMER,
        dwCommonButtons: TDCBF_CLOSE_BUTTON,
        pszWindowTitle: title.as_ptr(),
        pszMainInstruction: instruction.as_ptr(),
        pszContent: content.as_ptr(),
        pfCallback: Some(callback),
        lpCallbackData: (&raw mut state) as isize,
        ..Default::default()
    };
    // SAFETY: config, strings, and callback state remain live throughout the synchronous call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    eyre::ensure!(result >= 0, "TaskDialogIndirect failed: {result}");
    eyre::ensure!(
        state[3] == state[2],
        "dialog DPI mismatch: expected {}, observed {}",
        state[2],
        state[3]
    );
    writeln!(
        std::io::stdout().lock(),
        "PerMonitorV2 native dialog verified at ({}, {}): {} DPI ({}%)",
        state[0],
        state[1],
        state[3],
        state[3] * 100 / 96
    )?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
