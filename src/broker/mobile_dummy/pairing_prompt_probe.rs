use super::pairing_prompt_diagnostics::PairingPromptDiagnostics;
use super::pairing_prompt_probe_report::PairingPromptProbeReport;
use super::pairing_prompt_state::PairingPromptState;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use windows_sys::Win32::System::StationsAndDesktops::CloseDesktop;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READOBJECTS;
use windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop;
use windows_sys::Win32::System::StationsAndDesktops::OpenInputDesktop;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::System::Threading::GetStartupInfoW;
use windows_sys::Win32::System::Threading::STARTUPINFOW;
/// Exercises only the actual native comparison callback with fixed synthetic text on a private desktop.
/// No UKEY2 session, socket, worker, real code, request or lease is created.
/// # Errors
/// Fails closed unless the caller is on the exact generated private desktop and it is not the input desktop.
/// # Panics
/// Panics if this synthetic probe's private diagnostic mutexes have been poisoned.
pub fn run(
    expected_desktop: &str,
    presentation_enabled: bool,
) -> eyre::Result<PairingPromptProbeReport> {
    run_inner(expected_desktop, presentation_enabled, None)
}
/// Synthetic private-desktop proof of external resolution and actual dialog destruction.
/// # Errors
/// Refuses the input desktop, unknown private names, or failed native UI.
pub fn run_confirmation(
    expected_desktop: &str,
    accept: bool,
) -> eyre::Result<PairingPromptProbeReport> {
    run_inner(expected_desktop, true, Some(accept))
}
fn run_inner(
    expected_desktop: &str,
    presentation_enabled: bool,
    confirmation: Option<bool>,
) -> eyre::Result<PairingPromptProbeReport> {
    let suffix = expected_desktop
        .strip_prefix("teamy-broker-test-pairing-")
        .ok_or_else(|| eyre::eyre!("private synthetic desktop required"))?;
    uuid::Uuid::parse_str(suffix)
        .map_err(|_redacted_error| eyre::eyre!("private synthetic desktop required"))?;
    // SAFETY: own public thread identity and borrowed thread desktop; no switching or input.
    let thread = unsafe { GetCurrentThreadId() };
    // SAFETY: borrowed own-thread desktop handle, never closed here.
    let desktop = unsafe { GetThreadDesktop(thread) };
    let name = crate::platform::approval_preview_capture::object_name(desktop);
    // SAFETY: owned read-only input-desktop metadata handle, closed below.
    let input = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
    let input_name = crate::platform::approval_preview_capture::object_name(input);
    if !input.is_null() {
        // SAFETY: closes only this function's owned metadata handle.
        unsafe { CloseDesktop(input) };
    }
    eyre::ensure!(
        name == expected_desktop && input_name != "unavailable" && name != input_name,
        "private non-input desktop required"
    );
    let attempt = confirmation.map(|_| {
        crate::broker::phone::pairing_confirmation::Attempt::new(
            std::time::Instant::now() + std::time::Duration::from_secs(2),
            0,
        )
    });
    if let Some(a) = &attempt {
        a.publish(zeroize::Zeroizing::new(
            "SYNTHETIC PRIVATE-DESKTOP PROBE; NO LIVE COMPARISON CODE".into(),
        ))?;
    }
    let state = PairingPromptState {
        confirmation: attempt.as_ref().map(Arc::clone),
        code: zeroize::Zeroizing::new(
            "SYNTHETIC PRIVATE-DESKTOP PROBE; NO LIVE COMPARISON CODE".into(),
        ),
        until: std::time::Instant::now() + std::time::Duration::from_secs(2),
        closed: Arc::new(AtomicBool::new(false)),
    };
    let diagnostics = PairingPromptDiagnostics::default();
    let accepted = std::thread::scope(|scope| {
        if let (Some(accept), Some(attempt)) = (confirmation, attempt.as_ref()) {
            let diagnostics = &diagnostics;
            scope.spawn(move || {
                let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while std::time::Instant::now() < until {
                    if diagnostics
                        .events
                        .lock()
                        .expect("synthetic events")
                        .iter()
                        .any(|e| e.kind == crate::protocol::ApprovalPreviewEventKind::Created)
                    {
                        if accept {
                            let _ = attempt.confirm(
                                &attempt.id,
                                "SYNTHETIC PRIVATE-DESKTOP PROBE; NO LIVE COMPARISON CODE",
                            );
                        } else {
                            let _ = attempt.cancel_pending(&attempt.id);
                        }
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });
        }
        super::pairing_prompt::show_core(&state, Some(&diagnostics), presentation_enabled)
            .unwrap_or(false)
    });
    let mut startup = STARTUPINFOW::default();
    // SAFETY: writable own-process startup metadata, no foreign memory access.
    unsafe { GetStartupInfoW(&raw mut startup) };
    // SAFETY: reads this process's public numeric identity only.
    let process_pid = unsafe { GetCurrentProcessId() };
    Ok(PairingPromptProbeReport {
        process_pid,
        startup_flags: startup.dwFlags,
        startup_show_window: startup.wShowWindow,
        task_dialog_hresult: *diagnostics.hresult.lock().expect("synthetic result lock"),
        selected_button: *diagnostics.button.lock().expect("synthetic button lock"),
        accepted,
        presentation_enabled,
        protection_set: diagnostics
            .protection
            .into_inner()
            .expect("synthetic protection lock"),
        presentation: diagnostics
            .presentation
            .into_inner()
            .expect("synthetic presentation lock"),
        attention: diagnostics
            .attention
            .into_inner()
            .expect("synthetic attention lock"),
        events: diagnostics
            .events
            .into_inner()
            .expect("synthetic event lock"),
    })
}
