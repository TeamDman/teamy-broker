use crate::protocol::ApprovalAttentionOutcome;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::UI::WindowsAndMessaging::FLASHW_TRAY;
use windows_sys::Win32::UI::WindowsAndMessaging::FLASHWINFO;
use windows_sys::Win32::UI::WindowsAndMessaging::FlashWindowEx;
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION;
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

/// One attention request after presentation. Never retries focus or changes consent.
#[derive(Debug, Default)]
pub struct DialogAttention {
    requested: AtomicBool,
}

impl DialogAttention {
    pub(crate) fn request_if_visible(&self, hwnd: HWND) -> Option<ApprovalAttentionOutcome> {
        // SAFETY: Windows validates the live task-dialog HWND; no user content is read.
        let visible =
            unsafe { IsWindowVisible(hwnd) != 0 } && super::approval_input_desktop::matches(hwnd);
        self.request_using(
            visible,
            || {
                // SAFETY: Windows validates the live dialog HWND; no borrowed pointers.
                unsafe { SetForegroundWindow(hwnd) != 0 }
            },
            || {
                // SAFETY: returns an opaque public handle, compared only with this dialog.
                unsafe { GetForegroundWindow() == hwnd }
            },
            || {
                let cb_size = u32::try_from(std::mem::size_of::<FLASHWINFO>()).ok()?;
                let flash = FLASHWINFO {
                    cbSize: cb_size,
                    hwnd,
                    dwFlags: FLASHW_TRAY,
                    uCount: 1,
                    dwTimeout: 0,
                };
                // SAFETY: initialized FLASHWINFO stays live for this synchronous call.
                Some(unsafe { FlashWindowEx(&raw const flash) != 0 })
            },
            || {
                // SAFETY: requests a standard system sound, without input or consent action.
                let queued = unsafe { MessageBeep(MB_ICONINFORMATION) != 0 };
                if queued {
                    Ok(())
                } else {
                    // SAFETY: immediately reads this thread's documented MessageBeep failure code.
                    Err(unsafe { GetLastError() })
                }
            },
        )
    }

    fn request_using(
        &self,
        visible: bool,
        foreground: impl FnOnce() -> bool,
        matches: impl FnOnce() -> bool,
        flash: impl FnOnce() -> Option<bool>,
        sound: impl FnOnce() -> Result<(), u32>,
    ) -> Option<ApprovalAttentionOutcome> {
        if !visible || self.requested.swap(true, Ordering::AcqRel) {
            return None;
        }
        let foreground_returned = foreground();
        let foreground_matches = matches();
        let flash_previously_active = if foreground_matches { None } else { flash() };
        let (sound_queued, sound_error_code) = match sound() {
            Ok(()) => (true, None),
            Err(code) => (false, Some(code)),
        };
        Some(ApprovalAttentionOutcome {
            visible_before: visible,
            foreground_returned,
            foreground_matches,
            sound_queued,
            sound_error_code,
            flash_previously_active,
        })
    }
}

#[cfg(test)]
#[path = "approval_attention_tests.rs"]
mod tests;
