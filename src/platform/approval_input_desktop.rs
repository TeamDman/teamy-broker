use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::StationsAndDesktops::CloseDesktop;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READOBJECTS;
use windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop;
use windows_sys::Win32::System::StationsAndDesktops::OpenInputDesktop;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
pub(crate) fn matches(hwnd: HWND) -> bool {
    // SAFETY: reads the thread ID of this task dialog, without reading other UI.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    // SAFETY: borrowed handle of this dialog's thread desktop.
    let desktop = unsafe { GetThreadDesktop(thread) };
    // SAFETY: read-only input desktop metadata; owned handle closed below, never switched.
    let input = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
    let dialog_name = super::approval_preview_capture::object_name(desktop);
    let input_name = super::approval_preview_capture::object_name(input);
    let matched = dialog_name != "unavailable" && input_name == dialog_name;
    if !input.is_null() {
        // SAFETY: closes only the owned metadata handle from this function.
        unsafe { CloseDesktop(input) };
    }
    matched
}
