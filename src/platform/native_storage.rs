use super::dpapi::protect;
use crate::protocol::Backend;
use std::io::Write;
use windows_sys::Win32::Security::Credentials::CRED_PERSIST_LOCAL_MACHINE;
use windows_sys::Win32::Security::Credentials::CRED_TYPE_GENERIC;
use windows_sys::Win32::Security::Credentials::CREDENTIALW;
use windows_sys::Win32::Security::Credentials::CredDeleteW;
use windows_sys::Win32::Security::Credentials::CredWriteW;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn write(backend: &Backend, data: &[u8]) -> eyre::Result<()> {
    match backend {
        Backend::Memory => Ok(()),
        Backend::WindowsCredentialManager(target) => {
            eyre::ensure!(data.len() <= 2560, "credential blob too large");
            let name = super::identity::wide(target);
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: name.as_ptr().cast_mut(),
                CredentialBlobSize: u32::try_from(data.len())?,
                CredentialBlob: data.as_ptr().cast_mut(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                ..Default::default()
            };
            // SAFETY: synchronous CredWrite copies pointers backed by live input buffers.
            eyre::ensure!(
                // SAFETY: synchronous CredWrite copies pointers backed by live input buffers.
                unsafe { CredWriteW(&raw const credential, 0) } != 0,
                "credential write failed"
            );
            Ok(())
        }
        Backend::WellKnownFile(path) => {
            let encrypted = protect(data, false)?;
            let p = std::path::Path::new(path);
            let parent = p
                .parent()
                .ok_or_else(|| eyre::eyre!("cache parent missing"))?;
            eyre::ensure!(parent.is_dir(), "explicit cache parent must already exist");
            let temp = parent.join(format!(".teamy-broker-{}.tmp", uuid::Uuid::new_v4()));
            let result = (|| {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp)?;
                file.write_all(&encrypted)?;
                file.sync_all()?;
                drop(file);
                let source = super::identity::wide(&temp.to_string_lossy());
                let destination = super::identity::wide(path);
                // SAFETY: live NUL-terminated file paths; atomic replacement on the same volume.
                eyre::ensure!(
                    // SAFETY: live NUL-terminated file paths; atomic replacement on the same volume.
                    unsafe {
                        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                            source.as_ptr(),
                            destination.as_ptr(),
                            windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                                | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
                        )
                    } != 0,
                    "cache publish failed"
                );
                Ok(())
            })();
            let _ = std::fs::remove_file(&temp);
            result
        }
    }
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn delete(backend: &Backend) -> eyre::Result<()> {
    match backend {
        Backend::Memory => Ok(()),
        Backend::WellKnownFile(path) => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => eyre::bail!("cache removal failed"),
        },
        Backend::WindowsCredentialManager(target) => {
            let name = super::identity::wide(target);
            // SAFETY: valid NUL-terminated credential target.
            if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
                // SAFETY: inspect error immediately following the failing Win32 call.
                eyre::ensure!(
                    // SAFETY: inspect error immediately following the failing Win32 call.
                    unsafe { windows_sys::Win32::Foundation::GetLastError() } == 1168,
                    "credential removal failed"
                );
            }
            Ok(())
        }
    }
}
