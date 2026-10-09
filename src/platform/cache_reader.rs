use super::dpapi::protect;
use crate::protocol::Backend;
use std::io::Read;
use windows_sys::Win32::Security::Credentials::CRED_TYPE_GENERIC;
use windows_sys::Win32::Security::Credentials::CredFree;
use windows_sys::Win32::Security::Credentials::CredReadW;
use zeroize::Zeroizing;
/// Read the explicitly selected provider; never select a fallback.
/// # Errors
///
/// Returns an error on oversized, inaccessible, or invalid protected cache data.
pub fn read(backend: &Backend) -> eyre::Result<Option<Zeroizing<Vec<u8>>>> {
    match backend {
        Backend::Memory => Ok(None),
        Backend::WellKnownFile(path) => {
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_redacted_error) => eyre::bail!("cache read failed"),
            };
            let mut bytes = Vec::new();
            file.take(16385).read_to_end(&mut bytes)?;
            eyre::ensure!(bytes.len() <= 16384, "oversized cache");
            Ok(Some(protect(&bytes, true)?))
        }
        Backend::WindowsCredentialManager(target) => {
            let name = super::identity::wide(target);
            let mut credential = std::ptr::null_mut();
            // SAFETY: live NUL-terminated target and writable output pointer.
            let found =
                unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut credential) };
            if found == 0 {
                // SAFETY: query thread-local error immediately after the failed API.
                let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                eyre::ensure!(error == 1168, "cache read failed");
                return Ok(None);
            }
            // SAFETY: successful CredRead returned this owned CREDENTIALW allocation.
            let info = unsafe { &*credential };
            let len = info.CredentialBlobSize as usize;
            let bytes = if len == 0 {
                Zeroizing::new(Vec::new())
            } else {
                // SAFETY: the nonempty blob is valid for its Windows-provided size while credential is live.
                Zeroizing::new(
                    unsafe { std::slice::from_raw_parts(info.CredentialBlob, len) }.to_vec(),
                )
            };
            // SAFETY: free the exact allocation returned by CredRead.
            unsafe { CredFree(credential.cast()) };
            Ok(Some(bytes))
        }
    }
}
