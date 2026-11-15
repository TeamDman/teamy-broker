use std::ffi::OsStr;
use std::iter::repeat_n;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use windows::Win32::Foundation::HANDLE;
use windows::core::Owned;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::CreateProcessW;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTUPINFOW;
fn argument(output: &mut Vec<u16>, value: &OsStr) -> eyre::Result<()> {
    output.push(34);
    let mut slashes = 0;
    for unit in value.encode_wide() {
        eyre::ensure!(unit != 0, "invalid launch argument");
        if unit == 92 {
            slashes += 1;
            continue;
        }
        let count = if unit == 34 { slashes * 2 + 1 } else { slashes };
        output.extend(repeat_n(92, count));
        output.push(unit);
        slashes = 0;
    }
    output.extend(repeat_n(92, slashes * 2));
    output.push(34);
    Ok(())
}
/// Starts only the explicitly selected executable, without a shell or inherited caller handles.
/// # Errors
/// Returns a fixed redacted error on invalid arguments or failed process creation.
pub fn spawn(executable: &Path, arguments: &[String]) -> eyre::Result<()> {
    let mut command = Vec::new();
    argument(&mut command, executable.as_os_str())?;
    for value in arguments {
        command.push(32);
        argument(&mut command, OsStr::new(value))?;
    }
    eyre::ensure!(command.len() < 32767, "launch command too long");
    command.push(0);
    let startup = STARTUPINFOW {
        cb: u32::try_from(mem::size_of::<STARTUPINFOW>())?,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: writable UTF-16 command, live initialized structures; no inherited handles, custom environment or security attributes.
    let created = unsafe {
        CreateProcessW(
            ptr::null(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            CREATE_NO_WINDOW,
            ptr::null(),
            ptr::null(),
            &raw const startup,
            &raw mut process,
        )
    };
    eyre::ensure!(created != 0, "broker start failed");
    // SAFETY: successful CreateProcess transfers one owned thread handle.
    let _thread = unsafe { Owned::new(HANDLE(process.hThread)) };
    // SAFETY: the other successful result is an independently owned process handle.
    let _process = unsafe { Owned::new(HANDLE(process.hProcess)) };
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
            argument(&mut output, OsStr::new(input)).unwrap();
            assert_eq!(String::from_utf16(&output).unwrap(), expected);
        }
        let _ = argument(&mut Vec::new(), OsStr::new("bad\0argument")).unwrap_err();
    }
}
