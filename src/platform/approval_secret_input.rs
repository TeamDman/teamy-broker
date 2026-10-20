use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW;
use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowTextW;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_CHILD;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_TABSTOP;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_VISIBLE;
use zeroize::Zeroizing;
pub(crate) fn create(parent: HWND) -> Option<usize> {
    // SAFETY: live process-owned task dialog; capture exclusion protects user input.
    if unsafe { SetWindowDisplayAffinity(parent, 0x11) } == 0 {
        return None;
    }
    let mut rect = RECT::default();
    // SAFETY: writable rectangle and process-owned dialog.
    if unsafe { GetClientRect(parent, &raw mut rect) } == 0 {
        return None;
    }
    let class = super::identity::wide("EDIT");
    let empty = super::identity::wide("");
    // SAFETY: class/text buffers remain live; child is owned and destroyed by the dialog.
    let input = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            class.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x20 | 0x80,
            24,
            rect.bottom - 125,
            rect.right - 48,
            30,
            parent,
            103usize as _,
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if input.is_null() {
        return None;
    }
    // SAFETY: limits only our owned password edit control, no other application UI.
    unsafe { SendMessageW(input, 0x00C5, crate::protocol::MAX_SECRET, 0) };
    Some(input as usize)
}
pub(crate) fn read(handle: usize) -> Option<Zeroizing<Vec<u8>>> {
    let input = handle as HWND;
    let mut chars = Zeroizing::new(vec![0u16; crate::protocol::MAX_SECRET + 1]);
    // SAFETY: bounded writable buffer reads only this dialog's user-operated input.
    let count =
        unsafe { GetWindowTextW(input, chars.as_mut_ptr(), i32::try_from(chars.len()).ok()?) };
    clear(handle);
    let count = usize::try_from(count).ok()?;
    if count == 0 {
        return None;
    }
    let value = Zeroizing::new(String::from_utf16(&chars[..count]).ok()?);
    if value.len() > crate::protocol::MAX_SECRET {
        return None;
    }
    Some(Zeroizing::new(value.as_bytes().to_vec()))
}
pub(crate) fn clear(handle: usize) {
    let empty = super::identity::wide("");
    // SAFETY: clears only the owned password control while its parent is live.
    unsafe { SetWindowTextW(handle as HWND, empty.as_ptr()) };
}
