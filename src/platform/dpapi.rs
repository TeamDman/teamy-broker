use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;
use windows_sys::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN;
use windows_sys::Win32::Security::Cryptography::CryptProtectData;
use windows_sys::Win32::Security::Cryptography::CryptUnprotectData;
use zeroize::Zeroizing;
pub(crate) fn protect(data: &[u8], decrypt: bool) -> eyre::Result<Zeroizing<Vec<u8>>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = if decrypt {
        // SAFETY: input and output buffers live through the synchronous DPAPI call, with no UI or optional entropy.
        unsafe {
            CryptUnprotectData(
                &raw const input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &raw mut output,
            )
        }
    } else {
        // SAFETY: input and output buffers live through the synchronous DPAPI call, with no UI or optional entropy.
        unsafe {
            CryptProtectData(
                &raw const input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &raw mut output,
            )
        }
    };
    eyre::ensure!(ok != 0, "protected storage operation failed");
    let value = if output.cbData == 0 {
        Zeroizing::new(Vec::new())
    } else {
        // SAFETY: nonempty successful DPAPI result is valid for its returned length.
        Zeroizing::new(
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec(),
        )
    };
    if decrypt && output.cbData > 0 {
        // SAFETY: output is an owned writable buffer of the measured nonzero length.
        unsafe { std::ptr::write_bytes(output.pbData, 0, output.cbData as usize) };
    }
    // SAFETY: output was allocated by DPAPI; LocalFree accepts a null pointer for an empty result.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(value)
}
