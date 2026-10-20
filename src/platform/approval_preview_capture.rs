use crate::protocol::ApprovalPreviewEvent;
use crate::protocol::ApprovalPreviewEventKind;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Dwm::DwmGetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW;
use windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST;
use windows_sys::Win32::Graphics::Gdi::MONITORINFO;
use windows_sys::Win32::Graphics::Gdi::MonitorFromWindow;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::StationsAndDesktops::CloseDesktop;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READOBJECTS;
use windows_sys::Win32::System::StationsAndDesktops::GetProcessWindowStation;
use windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop;
use windows_sys::Win32::System::StationsAndDesktops::GetUserObjectInformationW;
use windows_sys::Win32::System::StationsAndDesktops::OpenInputDesktop;
use windows_sys::Win32::System::StationsAndDesktops::UOI_NAME;
use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

pub(crate) fn capture(
    hwnd: HWND,
    kind: ApprovalPreviewEventKind,
    button: Option<i32>,
) -> ApprovalPreviewEvent {
    let mut pid = 0;
    // SAFETY: Win32 validates the task-dialog HWND; output points to live writable storage.
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, &raw mut pid) };
    let mut session = 0;
    // SAFETY: PID is the observed window owner; output remains live for the call.
    let has_session = unsafe { ProcessIdToSessionId(pid, &raw mut session) != 0 };
    // SAFETY: returns the thread's borrowed desktop handle, never closed here.
    let desktop = object_name(unsafe { GetThreadDesktop(thread_id) });
    // SAFETY: returns this process's borrowed window-station handle.
    let window_station = object_name(unsafe { GetProcessWindowStation() });
    // SAFETY: read-only metadata access, no switching or input; handle is closed below.
    let input = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
    let input_desktop = object_name(input);
    if !input.is_null() {
        // SAFETY: this function owns the input desktop handle returned above.
        unsafe { CloseDesktop(input) };
    }
    let mut rect_value = zero_rect();
    // SAFETY: Win32 validates HWND; RECT output is live and writable.
    let rect =
        (unsafe { GetWindowRect(hwnd, &raw mut rect_value) } != 0).then(|| bounds(rect_value));
    // SAFETY: selects a monitor for metadata only; does not move the window.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).unwrap_or(0),
        rcMonitor: zero_rect(),
        rcWork: zero_rect(),
        dwFlags: 0,
    };
    // SAFETY: initialized MONITORINFO stays live; monitor handle is borrowed.
    let has_monitor = !monitor.is_null() && unsafe { GetMonitorInfoW(monitor, &raw mut info) != 0 };
    let monitor_rect = has_monitor.then(|| bounds(info.rcMonitor));
    let workarea_rect = has_monitor.then(|| bounds(info.rcWork));
    let mut cloak = 0_u32;
    // SAFETY: DWMWA_CLOAKED is a u32 result; no UI content is read.
    let cloak_result = unsafe { DwmGetWindowAttribute(hwnd, 14, (&raw mut cloak).cast(), 4) };
    // SAFETY: each function validates the HWND and only reads window state.
    let visible = unsafe { IsWindowVisible(hwnd) != 0 };
    // SAFETY: reads only minimized state of this task dialog.
    let minimized = unsafe { IsIconic(hwnd) != 0 };
    // SAFETY: compares an opaque public foreground handle; no unrelated UI content is read.
    let foreground_matches = unsafe { GetForegroundWindow() == hwnd };
    ApprovalPreviewEvent {
        kind,
        unix_ms: crate::protocol::wire::unix_ms().unwrap_or_default(),
        hwnd: u64::try_from(hwnd as usize).unwrap_or_default(),
        pid,
        thread_id,
        session_id: has_session.then_some(session),
        desktop,
        window_station,
        input_desktop,
        title: window_text(hwnd, false),
        class: window_text(hwnd, true),
        rect,
        monitor_rect,
        workarea_rect,
        monitor_intersects: intersects(rect, monitor_rect),
        workarea_intersects: intersects(rect, workarea_rect),
        visible,
        minimized,
        cloaked: (cloak_result >= 0).then_some(cloak),
        foreground_matches,
        button,
    }
}
fn zero_rect() -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}
fn bounds(rect: RECT) -> [i32; 4] {
    [rect.left, rect.top, rect.right, rect.bottom]
}
fn intersects(first: Option<[i32; 4]>, second: Option<[i32; 4]>) -> bool {
    match (first, second) {
        (Some(a), Some(b)) => a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3],
        _ => false,
    }
}
pub(crate) fn object_name(handle: HANDLE) -> String {
    let mut text = [0_u16; 256];
    let mut needed = 0;
    // SAFETY: buffer has exactly 512 bytes of writable storage; handle is read-only borrowed.
    let read = !handle.is_null()
        && unsafe {
            GetUserObjectInformationW(
                handle,
                UOI_NAME,
                text.as_mut_ptr().cast(),
                512,
                &raw mut needed,
            ) != 0
        };
    if read {
        String::from_utf16_lossy(
            &text[..text
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(text.len())],
        )
    } else {
        "unavailable".into()
    }
}
fn window_text(hwnd: HWND, class: bool) -> String {
    let mut text = [0_u16; 256];
    let count = if class {
        // SAFETY: fixed-size writable buffer; only this preview's HWND is supplied.
        unsafe { GetClassNameW(hwnd, text.as_mut_ptr(), 256) }
    } else {
        // SAFETY: only the fixed TEST/DUMMY dialog title is read, never credential UI or another app.
        unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), 256) }
    };
    let count = usize::try_from(count).unwrap_or_default().min(text.len());
    String::from_utf16_lossy(&text[..count])
}
