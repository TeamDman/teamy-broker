//! Private-desktop and loopback-only test fixture; never compiled into production.
use super::*;
use std::time::Duration;
use tokio::io::AsyncReadExt;
#[derive(facet::Facet, Debug)]
struct Event {
    label: String,
    elapsed_ms: u64,
}
pub(crate) struct Events {
    start: Instant,
    items: Mutex<Vec<Event>>,
}
impl Events {
    pub fn record(&self, label: &'static str) {
        if let Ok(mut events) = self.items.lock()
            && events.len() < 64
        {
            events.push(Event {
                label: label.into(),
                elapsed_ms: u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
        }
    }
    fn has(&self, label: &str) -> bool {
        self.items.lock().unwrap().iter().any(|e| e.label == label)
    }
}
fn private_desktop() -> eyre::Result<()> {
    use windows_sys::Win32::System::StationsAndDesktops::CloseDesktop;
    use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READOBJECTS;
    use windows_sys::Win32::System::StationsAndDesktops::GetThreadDesktop;
    use windows_sys::Win32::System::StationsAndDesktops::OpenInputDesktop;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    let expected = std::env::var("TEAMY_FAITHFUL_PRIVATE_DESKTOP")?;
    let suffix = expected
        .strip_prefix("teamy-broker-test-pairing-")
        .ok_or_else(|| eyre::eyre!("private test desktop required"))?;
    uuid::Uuid::parse_str(suffix)?;
    // SAFETY: metadata reads of this test's own desktop; owned input metadata handle closed below.
    let thread = unsafe { GetCurrentThreadId() };
    // SAFETY: querying the desktop of this test thread.
    let desktop = unsafe { GetThreadDesktop(thread) };
    // SAFETY: opens a metadata-only input desktop handle.
    let input = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
    let actual = crate::platform::approval_preview_capture::object_name(desktop);
    let input_name = crate::platform::approval_preview_capture::object_name(input);
    if !input.is_null() {
        // SAFETY: closes the owned metadata handle.
        unsafe { CloseDesktop(input) };
    }
    eyre::ensure!(
        actual == expected && actual != input_name && input_name != "unavailable",
        "private non-input desktop required"
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires explicitly isolated private desktop and emulator peer; no live enrollment"]
#[expect(
    clippy::too_many_lines,
    reason = "Single bounded integration scenario keeps cleanup and assertions together"
)]
async fn faithful_listener_android_two_minute_delay() -> eyre::Result<()> {
    private_desktop()?;
    let fixture = super::tests::Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let id = controller
        .status()?
        .pairing_attempt_id
        .ok_or_else(|| eyre::eyre!("test attempt missing"))?;
    let events = Arc::new(Events {
        start: Instant::now(),
        items: Mutex::new(Vec::new()),
    });
    let (ready, receive) = tokio::sync::oneshot::channel();
    let policy = super::resident::NetworkPolicy::Loopback {
        ready: Arc::new(Mutex::new(Some(ready))),
        events: Arc::clone(&events),
    };
    let engine = Arc::new(crate::broker::engine::Engine::new(
        Arc::new(crate::broker::mobile_dummy::adapter::MobileDummyAdapter),
        Arc::new(crate::broker::native_approval::NativeApproval),
        Arc::new(crate::broker::mobile_dummy::storage::DummyMemoryStorage),
    ));
    let cancel = teamy_cancellation::CancellationToken::default();
    let owned = Arc::clone(&controller);
    let work = Arc::clone(&engine);
    let stop = cancel.clone();
    let actor = tokio::spawn(async move {
        let run = Arc::clone(&owned);
        super::resident::guarded(owned, super::resident::run_inner(run, stop, work, policy)).await
    });
    let port = tokio::time::timeout(Duration::from_secs(5), receive).await??;
    let control = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let control_port = control.local_addr()?.port();
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!(
            "{{\"loopback_port\":{port},\"control_port\":{control_port},\"test_only\":true}}\n"
        )
        .as_bytes(),
    )?;
    let (mut phone, _) = tokio::time::timeout(Duration::from_mins(1), control.accept()).await??;
    drop(control);
    let mut length = [0; 4];

    phone.read_exact(&mut length).await?;
    eyre::ensure!(
        u32::from_be_bytes(length) == 19,
        "synthetic phone comparison shape"
    );
    let mut code = zeroize::Zeroizing::new(vec![0; 19]);
    phone.read_exact(&mut code).await?;
    let code = crate::protocol::pairing_comparison::PairingCode {
        comparison: std::str::from_utf8(&code)?.into(),
    };
    drop(phone);
    events.record("independent_phone_code_received");
    let began = Instant::now();
    while began.elapsed() < Duration::from_mins(2) {
        if !controller.status()?.pairing_window_open {
            let d = controller.status()?.diagnostic;
            std::io::Write::write_all(
                &mut std::io::stdout(),
                facet_json::to_string(&d)?.as_bytes(),
            )?;
            eyre::bail!("actor closed during synthetic human wait");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    eyre::ensure!(
        events.has("native_created") && !events.has("native_destroyed"),
        "native comparison must remain open"
    );
    events.record("two_minute_native_wait_complete");
    let endpoint = format!(r"\\.\pipe\faithful-pairing-test-{}", uuid::Uuid::new_v4());
    let mut pipe = crate::platform::pipe_security::create(&endpoint, true)?;
    let c = Arc::clone(&controller);
    let dispatch = tokio::spawn(async move {
        pipe.connect().await?;
        let hello = crate::protocol::Hello {
            version: crate::protocol::VERSION,
            instance_id: "synthetic".into(),
            capabilities: crate::protocol::CAPABILITIES
                .iter()
                .map(|v| (*v).into())
                .chain(std::iter::once("persistent-no-vault-phone-v1".into()))
                .collect(),
        };
        crate::protocol::wire::write(&mut pipe, &hello).await?;
        let peer: crate::protocol::Hello = crate::protocol::wire::read(&mut pipe).await?;
        let request: crate::protocol::Request = crate::protocol::wire::read(&mut pipe).await?;
        request.validate()?;
        crate::broker::phone_control_session::execute_with(&mut pipe, &peer, &request, &c).await
    });
    let client = crate::sdk::Client::new(None)?.with_endpoint(endpoint);
    eyre::ensure!(
        client.pairing_confirm(&id, &code).await?.accepted,
        "independent synthetic confirmation rejected"
    );
    dispatch.await??;
    events.record("scoped_pipe_confirmation_accepted");
    let until = Instant::now() + Duration::from_secs(10);
    while !events.has("socket_dropped_before_enrollment") && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    eyre::ensure!(
        events.has("native_destroyed") && events.has("socket_dropped_before_enrollment"),
        "native/socket cleanup missing"
    );
    eyre::ensure!(!controller.status()?.paired, "test enrolled identity");
    eyre::ensure!(engine.leases.lock().await.is_empty(), "test created lease");
    cancel.request_cancel("synthetic test complete");
    actor.await??;
    eyre::ensure!(!controller.status()?.listener, "test listener leaked");
    events.record("listener_stopped");
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!(
            "\n{{\"faithful_success\":true,\"enrollment\":false,\"events\":{}}}\n",
            facet_json::to_string(&*events.items.lock().unwrap())?
        )
        .as_bytes(),
    )?;
    Ok(())
}
