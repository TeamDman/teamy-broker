use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::net::windows::named_pipe::ServerOptions;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn create(endpoint: &str, first: bool) -> eyre::Result<NamedPipeServer> {
    tracing::debug!(phase = "pipe-owner-query", "Broker pipe setup");
    let sddl = super::identity::wide(&format!("D:P(A;;GA;;;{})", super::identity::sid()?));
    tracing::debug!(phase = "acl-create", "Broker pipe setup");
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: NUL-terminated SDDL and valid descriptor output pointer.
    eyre::ensure!(
        // SAFETY: NUL-terminated SDDL and valid descriptor output pointer.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &raw mut descriptor,
                std::ptr::null_mut(),
            )
        } != 0,
        "pipe security unavailable"
    );
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>())?,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    tracing::debug!(phase = "pipe-create", "Broker pipe setup");
    // SAFETY: attributes and descriptor remain live through CreateNamedPipe; Windows copies the security descriptor.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(endpoint, (&raw mut attributes).cast())
    };
    // SAFETY: descriptor allocation was returned by the SDDL conversion.
    unsafe { LocalFree(descriptor) };
    tracing::debug!(
        phase = "pipe-create-complete",
        success = result.is_ok(),
        "Broker pipe setup"
    );
    Ok(result?)
}
