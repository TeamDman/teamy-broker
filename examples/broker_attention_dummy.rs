//! Explicit standalone native attention check. Never creates an Engine, lease, or vault adapter.
use std::env;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::panic;
use std::process;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use teamy_broker::platform::native_approval;
use teamy_broker::protocol::AcquireRequest;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::FailureKind;
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
fn main() {
    panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    if env::args_os().skip(1).collect::<Vec<_>>() != [OsString::from("--show-harmless-attention")] {
        Write::write_all(&mut io::stdout().lock(),b"PROPOSAL ONLY: standalone synthetic native approval window, no 1Password, no broker connection, no lease/value. Explicit --show-harmless-attention shows it for at most 20 seconds. Choose Deny.\n")?;
        return Ok(());
    }
    // Only the explicitly requested synthetic UI check emits its nonsecret observations.
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .try_init()
        .map_err(|_error| eyre::eyre!("harmless attention diagnostics unavailable"))?;
    let cancel = Arc::new(AtomicBool::new(false));
    let foreground = Arc::new(AtomicBool::new(false));
    let ending = Arc::clone(&cancel);
    let focused = Arc::clone(&foreground);
    let pid = process::id();
    let watcher = thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        // SAFETY: Windows returns a public foreground HWND; no user content is read or acted on.
        let hwnd = unsafe { GetForegroundWindow() };
        let mut owner = 0;
        // SAFETY: owner is live writable storage, and Windows validates the HWND.
        unsafe { GetWindowThreadProcessId(hwnd, &raw mut owner) };
        focused.store(owner == pid, Ordering::Release);
        thread::sleep(Duration::from_secs(19));
        ending.store(true, Ordering::Release);
    });
    let request=AcquireRequest{
        account:"synthetic-no-vault".into(),secret_ref:"op://Synthetic/attention/no-secret".into(),
        purpose:"Harmless attention UI check only. No 1Password call or lease/value is possible. Choose Deny; this window closes within 20 seconds.".into(),
        requesting_app:"teamy-broker attention check (DUMMY ONLY)".into(),destination:"https://example.invalid".into(),backend:Backend::Memory,
    };
    let result = native_approval::approve(&request, pid, &cancel);
    let _ = watcher.join();
    if let Err(error) = result {
        eyre::ensure!(
            error.downcast_ref::<FailureKind>() == Some(&FailureKind::RequestCancelled),
            "harmless attention window unavailable"
        );
    }
    let receipt = format!(
        "Harmless native attention check closed; own window foreground at 1s: {}. No vault call, broker connection, lease or value. Audible sound requires user confirmation.\n",
        foreground.load(Ordering::Acquire)
    );
    Write::write_all(&mut io::stdout().lock(), receipt.as_bytes())?;
    Ok(())
}
