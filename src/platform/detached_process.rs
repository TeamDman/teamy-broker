use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::CreateProcessW;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTUPINFOW;
fn argument(output: &mut Vec<u16>, value: &std::ffi::OsStr) -> eyre::Result<()> {
    output.push(34);
    let mut slashes = 0;
    for unit in value.encode_wide() {
        eyre::ensure!(unit != 0, "invalid launch argument");
        if unit == 92 {
            slashes += 1;
            continue;
        }
        let count = if unit == 34 { slashes * 2 + 1 } else { slashes };
        output.extend(std::iter::repeat_n(92, count));
        output.push(unit);
        slashes = 0;
    }
    output.extend(std::iter::repeat_n(92, slashes * 2));
    output.push(34);
    Ok(())
}
/// Starts only the explicitly selected executable, without a shell or inherited caller handles.
/// # Errors
/// Returns a fixed redacted error on invalid arguments or failed process creation.
pub fn spawn(executable: &std::path::Path, arguments: &[String]) -> eyre::Result<()> {
    let mut command = Vec::new();
    argument(&mut command, executable.as_os_str())?;
    for value in arguments {
        command.push(32);
        argument(&mut command, std::ffi::OsStr::new(value))?;
    }
    eyre::ensure!(command.len() < 32767, "launch command too long");
    command.push(0);
    let startup = STARTUPINFOW {
        cb: u32::try_from(std::mem::size_of::<STARTUPINFOW>())?,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: writable UTF-16 command, live initialized structures; no inherited handles, custom environment or security attributes.
    let created = unsafe {
        CreateProcessW(
            std::ptr::null(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &raw const startup,
            &raw mut process,
        )
    };
    eyre::ensure!(created != 0, "broker start failed");
    // SAFETY: successful CreateProcess returns owned process and thread handles; child remains independent after closing them.
    unsafe { CloseHandle(process.hThread) };
    // SAFETY: close the other owned handle exactly once without terminating the normal per-user child.
    unsafe { CloseHandle(process.hProcess) };
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoting_preserves_empty_spaces_quotes_and_trailing_slashes_and_rejects_nul() {
        for (input, expected) in [
            ("", "\"\""),
            ("with space", "\"with space\""),
            ("a\"b", "\"a\\\"b\""),
            ("tail\\", "\"tail\\\\\""),
        ] {
            let mut output = Vec::new();
            argument(&mut output, std::ffi::OsStr::new(input)).unwrap();
            assert_eq!(String::from_utf16(&output).unwrap(), expected);
        }
        let _ = argument(&mut Vec::new(), std::ffi::OsStr::new("bad\0argument")).unwrap_err();
    }
}
