use windows_sys::Win32::UI::Controls::TASKDIALOGCONFIG;
use windows_sys::Win32::UI::Controls::TDCBF_OK_BUTTON;
use windows_sys::Win32::UI::Controls::TaskDialogIndirect;
use zeroize::Zeroizing;
/// Show a temporary shared session key only in an explicit local native bootstrap UI.
/// Synthetic-only: this is not a persistent pairing or production identity mechanism.
/// # Errors
/// Returns a redacted error if the native dialog cannot be shown.
pub fn show(session: &teamy_broker_handoff::session::Session) -> eyre::Result<()> {
    show_transport(session, None)
}
/// # Errors
/// Returns a redacted error if the synthetic native bootstrap UI cannot be shown.
pub fn show_with_tls(
    session: &teamy_broker_handoff::session::Session,
    fingerprint: &str,
) -> eyre::Result<()> {
    show_transport(session, Some(fingerprint))
}
fn show_transport(
    session: &teamy_broker_handoff::session::Session,
    fingerprint: Option<&str>,
) -> eyre::Result<()> {
    let code = session.bootstrap();
    let content = Zeroizing::new(format!(
        "DUMMY ONLY — temporary approval session.\n\nEnter this temporary session key in the Android app, then dismiss:\n\n{}\n\nDesktop: {}\nDesktop and phone receive one shared pending request. Either can supply the fixed dummy value.\nFor a separately approved TLS bridge test, also enter the temporary certificate pin below.\nNo persistent pairing. No vault access. Never enter a real credential.\nThe request expires 60 seconds after you dismiss this dialog.",
        *code,
        teamy_broker_handoff::DESKTOP
    ));
    let content = Zeroizing::new(match fingerprint {
        Some(pin) => format!(
            "{}\n\nTemporary TLS endpoint: {}:{}\nPermitted phone: {}\nTLS certificate SHA256 pin (enter exactly):\n{}\n\nCertificate/session expire; no OS trust or persistent key storage. Refresh after dismissing.",
            *content,
            super::tailnet_peer::DESKTOP,
            super::tailnet_peer::PORT,
            super::tailnet_peer::PHONE,
            pin
        ),
        None => (*content).clone(),
    });
    let wide: Zeroizing<Vec<u16>> = Zeroizing::new(content.encode_utf16().chain(Some(0)).collect());
    let title: Vec<u16> = "teamy-broker DUMMY ONLY\0".encode_utf16().collect();
    let config = TASKDIALOGCONFIG {
        cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())?,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: title.as_ptr(),
        pszContent: wide.as_ptr(),
        ..Default::default()
    };
    // SAFETY: configuration and all UTF-16 buffers live throughout the synchronous dialog call.
    let result = unsafe {
        TaskDialogIndirect(
            &raw const config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    eyre::ensure!(result >= 0, "bootstrap UI unavailable");
    Ok(())
}
