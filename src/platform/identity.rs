use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenUser;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn sid() -> eyre::Result<String> {
    // SAFETY: Windows current-process pseudo-handle requires no close.
    let process = unsafe { GetCurrentProcess() };
    let mut token = std::ptr::null_mut();
    // SAFETY: valid current-process pseudo handle and writable output handle.
    eyre::ensure!(
        // SAFETY: valid current-process pseudo handle and writable output handle.
        unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) } != 0,
        "identity unavailable"
    );
    let result = (|| {
        let mut needed = 0;
        // SAFETY: size query uses null buffer with zero length.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &raw mut needed) };
        let mut data = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: aligned buffer sized by the token information query.
        eyre::ensure!(
            // SAFETY: aligned buffer sized by the token information query.
            unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    data.as_mut_ptr().cast(),
                    needed,
                    &raw mut needed,
                )
            } != 0,
            "identity unavailable"
        );
        // SAFETY: successful TokenUser query returns TOKEN_USER in aligned buffer.
        let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
        let mut text = std::ptr::null_mut();
        // SAFETY: SID belongs to the live token query buffer, output receives a LocalAlloc string.
        eyre::ensure!(
            // SAFETY: SID belongs to the live token query buffer, output receives a LocalAlloc string.
            unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) } != 0,
            "identity unavailable"
        );
        let mut len = 0;
        // SAFETY: the returned SID string is NUL terminated.
        loop {
            // SAFETY: pointer advances within the NUL-terminated Windows SID allocation.
            let pointer = unsafe { text.add(len) };
            // SAFETY: pointer is within that live string allocation.
            if unsafe { *pointer } == 0 {
                break;
            }
            len += 1;
        }
        // SAFETY: length was measured within the allocated NUL-terminated string.
        let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, len) });
        // SAFETY: this allocation was returned by ConvertSidToStringSidW.
        unsafe { LocalFree(text.cast()) };
        Ok(value?)
    })();
    // SAFETY: owned token handle is closed exactly once.
    unsafe { CloseHandle(token) };
    result
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn endpoint() -> eyre::Result<String> {
    Ok(format!(r"\\.\pipe\teamy-broker-v1-{}", sid()?))
}
#[must_use]
pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn lease_root() -> eyre::Result<std::path::PathBuf> {
    let local = directories_next::BaseDirs::new()
        .ok_or_else(|| eyre::eyre!("user directories unavailable"))?;
    Ok(local
        .data_local_dir()
        .join("Teamy")
        .join("teamy-broker")
        .join("leases"))
}
