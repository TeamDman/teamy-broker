use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
pub(crate) fn update(hwnd: HWND, shared: &crate::broker::shared_preview::SharedPreview) {
    let instruction = super::identity::wide(if shared.synthetic_fixture() {
        "ISOLATED TEST: enter only teamy-fixture-input-v1; fixture provider never reads a vault"
    } else if shared
        .provider_failed
        .load(std::sync::atomic::Ordering::Acquire)
    {
        "Desktop read failed. You can still supply or deny."
    } else if shared
        .provider_requested
        .load(std::sync::atomic::Ordering::Acquire)
    {
        "Desktop 1Password is waiting. Supply remains available."
    } else {
        "Choose how to provide this exact credential"
    });
    // SAFETY: updates public instruction text on our owned live dialog; never a credential value.
    unsafe {
        SendMessageW(
            hwnd,
            u32::try_from(windows_sys::Win32::UI::Controls::TDM_SET_ELEMENT_TEXT)
                .expect("message constant"),
            usize::try_from(windows_sys::Win32::UI::Controls::TDE_MAIN_INSTRUCTION)
                .expect("instruction constant"),
            instruction.as_ptr() as isize,
        )
    };
}
