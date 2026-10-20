use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use teamy_broker::broker::adapter::Adapter;
use teamy_broker::broker::credential_approvals;
use teamy_broker::broker::one_password::OnePassword;
use teamy_broker::broker::shared_preview::SharedPreviewHub;
use teamy_broker::protocol::AcquireRequest;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::credential_context::CredentialContext;
#[tokio::test]
async fn explicit_supply_terminates_the_owned_synthetic_op_child() -> eyre::Result<()> {
    let dir =
        std::env::temp_dir().join(format!("teamy-synthetic-provider-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir)?;
    let marker = dir.join("synthetic-provider.pid");
    let exe = std::env::current_exe()?
        .parent()
        .ok_or_else(|| eyre::eyre!("test path"))?
        .parent()
        .ok_or_else(|| eyre::eyre!("test path"))?
        .join("examples/synthetic_op_wait.exe");
    eyre::ensure!(
        exe.is_file(),
        "build the offline synthetic_op_wait example before this test"
    );
    let request = AcquireRequest {
        account: marker.to_string_lossy().into_owned(),
        secret_ref: "op://Synthetic/Example/token".into(),
        requesting_app: "synthetic-discord".into(),
        purpose: "dummy-only process cancellation".into(),
        destination: "https://discord.invalid/dummy".into(),
        backend: Backend::Memory,
    };
    let hub = SharedPreviewHub::default();
    let shared = hub.begin_credential(
        Arc::new(AtomicBool::new(false)),
        CredentialContext {
            request: request.clone(),
            supply: true,
            connected_process_id: 1,
            provider_state: "idle".into(),
            synthetic_fixture: false,
        },
    )?;
    assert!(shared.request_provider());
    let running = Arc::clone(&shared);
    let task = tokio::spawn(async move {
        let adapter = OnePassword { executable: exe };
        credential_approvals::resolve(running, adapter.read(&request)).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !marker.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await?;
    let pid: u32 = std::fs::read_to_string(&marker)?.parse()?;
    // SAFETY: observe termination only for the PID emitted by our own synthetic child.
    let process = unsafe {
        windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    eyre::ensure!(
        !process.is_null(),
        "synthetic process observation unavailable"
    );
    assert!(shared.supply(zeroize::Zeroizing::new(
        b"synthetic-phone-fallback".to_vec()
    )));
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(3), task).await???;
    assert_eq!(&*bytes, b"synthetic-phone-fallback");
    // SAFETY: valid owned process observation handle, bounded wait for kill_on_drop.
    let wait = unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(process, 3000) };
    // SAFETY: closes only the observation handle opened above.
    unsafe { windows_sys::Win32::Foundation::CloseHandle(process) };
    assert_eq!(wait, windows_sys::Win32::Foundation::WAIT_OBJECT_0);
    std::fs::remove_file(marker)?;
    std::fs::remove_dir(dir)?;
    Ok(())
}
