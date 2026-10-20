use crate::protocol::ApprovalPresentationOutcome;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE;
use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE;
use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOSIZE;
use windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER;
use windows_sys::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW;
use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos;
#[derive(Debug, Default)]
pub(crate) struct DialogPresentation {
    attempted: AtomicBool,
}
impl DialogPresentation {
    pub fn show_if_needed(&self, hwnd: HWND) -> Option<ApprovalPresentationOutcome> {
        self.present_using(
            || {
                // SAFETY: callback supplies the live native task-dialog HWND; reads state only.
                unsafe { IsWindowVisible(hwnd) != 0 }
            },
            || {
                // SAFETY: shows only this owner-thread dialog, without activation, movement, resize or Z-order changes.
                let shown = unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                    ) != 0
                };
                if shown {
                    Ok(())
                } else {
                    // SAFETY: immediately reads the documented failure code on the same thread.
                    Err(unsafe { GetLastError() })
                }
            },
        )
    }
    fn present_using(
        &self,
        visible: impl Fn() -> bool,
        show: impl FnOnce() -> Result<(), u32>,
    ) -> Option<ApprovalPresentationOutcome> {
        if self.attempted.swap(true, Ordering::AcqRel) {
            return None;
        }
        let visible_before = visible();
        let result = (!visible_before).then(show);
        let show_succeeded = result.as_ref().map(Result::is_ok);
        let show_error_code = result.and_then(Result::err);
        Some(ApprovalPresentationOutcome {
            visible_before,
            show_requested: !visible_before,
            show_succeeded,
            show_error_code,
            visible_after: visible(),
        })
    }
}
#[cfg(test)]
#[path = "approval_presentation_tests.rs"]
mod tests;
