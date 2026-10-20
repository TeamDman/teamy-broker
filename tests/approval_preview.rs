#[path = "support/fake_adapter.rs"]
mod fake_adapter;
#[path = "support/fake_preview_approval.rs"]
mod fake_preview_approval;
#[path = "support/fake_storage.rs"]
mod fake_storage;
use fake_adapter::FakeAdapter;
use fake_preview_approval::FakePreviewApproval;
use fake_storage::FakeStorage;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::protocol::ApprovalPreviewDisposition;
use teamy_broker::sdk::Client;

#[test]
fn rich_preview_diagnostics_roundtrip_without_a_secret_or_lease_shape() {
    use teamy_broker::protocol::ApprovalPreviewEvent;
    use teamy_broker::protocol::ApprovalPreviewEventKind;
    let event = ApprovalPreviewEvent {
        kind: ApprovalPreviewEventKind::ButtonClicked,
        unix_ms: 1,
        hwnd: 42,
        pid: 123,
        thread_id: 456,
        session_id: Some(1),
        desktop: "Default".into(),
        window_station: "WinSta0".into(),
        input_desktop: "Default".into(),
        title: "TEST/DUMMY".into(),
        class: "#32770".into(),
        rect: Some([-640, 20, -10, 400]),
        monitor_rect: Some([-1920, 0, 0, 1080]),
        workarea_rect: Some([-1920, 0, 0, 1040]),
        monitor_intersects: true,
        workarea_intersects: true,
        visible: true,
        minimized: false,
        cloaked: Some(0),
        foreground_matches: true,
        button: Some(101),
    };
    let encoded = facet_json::to_string(&event).unwrap();
    let decoded: ApprovalPreviewEvent = facet_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.rect, Some([-640, 20, -10, 400]));
    assert_eq!(decoded.kind, ApprovalPreviewEventKind::ButtonClicked);
    assert_eq!(decoded.button, Some(101));
    assert!(
        !encoded.contains("secret_ref")
            && !encoded.contains("lease_id")
            && !encoded.contains("token")
    );
}

#[tokio::test]
async fn approved_preview_sends_only_metadata_and_then_eof() {
    use tokio::io::AsyncReadExt;
    let approval = Arc::new(FakePreviewApproval::new(
        ApprovalPreviewDisposition::ApprovedTest,
    ));
    let adapter = Arc::new(FakeAdapter::default());
    let storage = Arc::new(FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        approval,
        Arc::<FakeStorage>::clone(&storage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-preview-{}",
        uuid::Uuid::new_v4()
    );
    let server = tokio::spawn({
        let endpoint = endpoint.clone();
        async move {
            teamy_broker::broker::server::serve_at(
                engine,
                teamy_cancellation::CancellationToken::new(),
                &endpoint,
                false,
            )
            .await
        }
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint.clone())
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut pipe = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(&endpoint)
        .unwrap();
    let hello: teamy_broker::protocol::Hello =
        teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
    assert!(
        hello
            .capabilities
            .iter()
            .any(|cap| cap == teamy_broker::protocol::APPROVAL_PREVIEW_CAPABILITY)
    );
    teamy_broker::protocol::wire::write(&mut pipe, &hello)
        .await
        .unwrap();
    teamy_broker::protocol::wire::write(
        &mut pipe,
        &teamy_broker::protocol::Request::control("approval-preview", None),
    )
    .await
    .unwrap();
    let reply: teamy_broker::protocol::Response =
        teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
    assert_eq!(reply.code, "preview-complete");
    assert!(
        reply.lease_id.is_none()
            && reply.expires_unix_ms.is_none()
            && reply.remaining_millis.is_none()
    );
    let result: teamy_broker::protocol::ApprovalPreviewResult =
        teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
    assert_eq!(result.disposition, ApprovalPreviewDisposition::ApprovedTest);
    let _ = tokio::time::timeout(Duration::from_secs(2), pipe.read_u8())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
    assert_eq!(client.status().await.unwrap().active_leases, 0);
    client.stop().await.unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn every_preview_disposition_is_isolated_from_approval_vault_storage_and_leases() {
    for disposition in [
        ApprovalPreviewDisposition::ApprovedTest,
        ApprovalPreviewDisposition::DeniedTest,
        ApprovalPreviewDisposition::Closed,
        ApprovalPreviewDisposition::Cancelled,
        ApprovalPreviewDisposition::TimedOut,
        ApprovalPreviewDisposition::Unavailable,
    ] {
        let adapter = Arc::new(FakeAdapter::default());
        let storage = Arc::new(FakeStorage::default());
        let approval = Arc::new(FakePreviewApproval::new(disposition));
        let engine = Engine::new(
            Arc::<FakeAdapter>::clone(&adapter),
            Arc::<FakePreviewApproval>::clone(&approval),
            Arc::<FakeStorage>::clone(&storage),
        );
        let result = engine
            .approval_preview(123, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_eq!(result.disposition, disposition);
        assert_eq!(result.connected_client_pid, 123);
        assert_eq!(approval.calls.load(Ordering::SeqCst), 1);
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
        assert!(storage.published.lock().unwrap().is_empty());
        assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
        let state = engine.status().await.unwrap();
        assert_eq!(state.active_leases, 0);
        assert_eq!(state.active_requests, 0);
        assert_eq!(state.active_approvals, 0);
        assert_eq!(state.open_approval_dialogs, 0);
        assert!(state.safe_to_stop);
    }
}
#[tokio::test]
async fn cancelled_preview_never_starts_native_ui_or_any_provider() {
    let approval = Arc::new(FakePreviewApproval::new(
        ApprovalPreviewDisposition::ApprovedTest,
    ));
    let adapter = Arc::new(FakeAdapter::default());
    let storage = Arc::new(FakeStorage::default());
    let engine = Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::<FakePreviewApproval>::clone(&approval),
        Arc::<FakeStorage>::clone(&storage),
    );
    assert_eq!(
        engine
            .approval_preview(1, Arc::new(AtomicBool::new(true)))
            .await
            .unwrap_err()
            .downcast_ref::<teamy_broker::protocol::FailureKind>(),
        Some(&teamy_broker::protocol::FailureKind::RequestCancelled)
    );
    assert_eq!(approval.calls.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn ipc_preview_tracks_dialog_rejects_duplicates_and_returns_no_secret_frame_or_lease() {
    let release = Arc::new(tokio::sync::Notify::new());
    let approval = Arc::new(FakePreviewApproval {
        release: Some(Arc::clone(&release)),
        ..FakePreviewApproval::new(ApprovalPreviewDisposition::ApprovedTest)
    });
    let adapter = Arc::new(FakeAdapter::default());
    let storage = Arc::new(FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::<FakePreviewApproval>::clone(&approval),
        Arc::<FakeStorage>::clone(&storage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-preview-{}",
        uuid::Uuid::new_v4()
    );
    let server = tokio::spawn({
        let endpoint = endpoint.clone();
        async move {
            teamy_broker::broker::server::serve_at(
                engine,
                teamy_cancellation::CancellationToken::new(),
                &endpoint,
                false,
            )
            .await
        }
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let first = tokio::spawn({
        let client = client.clone();
        async move { client.approval_preview().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while approval.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let state = client.status().await.unwrap();
    assert_eq!(state.open_approval_dialogs, 1);
    assert_eq!(state.active_requests, 1);
    assert!(!state.safe_to_stop);
    assert_eq!(
        client
            .approval_preview()
            .await
            .unwrap_err()
            .downcast_ref::<teamy_broker::protocol::FailureKind>(),
        Some(&teamy_broker::protocol::FailureKind::ShutdownBlocked)
    );
    assert_eq!(
        client
            .stop()
            .await
            .unwrap_err()
            .downcast_ref::<teamy_broker::protocol::FailureKind>(),
        Some(&teamy_broker::protocol::FailureKind::ShutdownBlocked)
    );
    release.notify_one();
    let result = first.await.unwrap().unwrap();
    assert_eq!(result.disposition, ApprovalPreviewDisposition::ApprovedTest);
    assert_eq!(result.connected_client_pid, std::process::id());
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
    let state = client.status().await.unwrap();
    assert_eq!(state.active_leases, 0);
    client.stop().await.unwrap();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn client_disconnect_cancels_preview_and_releases_work_without_provider_calls() {
    let approval = Arc::new(FakePreviewApproval {
        release: Some(Arc::new(tokio::sync::Notify::new())),
        ..FakePreviewApproval::new(ApprovalPreviewDisposition::ApprovedTest)
    });
    let adapter = Arc::new(FakeAdapter::default());
    let storage = Arc::new(FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::<FakePreviewApproval>::clone(&approval),
        Arc::<FakeStorage>::clone(&storage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-preview-{}",
        uuid::Uuid::new_v4()
    );
    let server = tokio::spawn({
        let endpoint = endpoint.clone();
        async move {
            teamy_broker::broker::server::serve_at(
                engine,
                teamy_cancellation::CancellationToken::new(),
                &endpoint,
                false,
            )
            .await
        }
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let task = tokio::spawn({
        let client = client.clone();
        async move { client.approval_preview().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while approval.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        while !client.status().await.unwrap().safe_to_stop {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        approval
            .last_cancel
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst)
    );
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
    assert_eq!(client.status().await.unwrap().active_leases, 0);
    client.stop().await.unwrap();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn legacy_broker_is_rejected_before_any_preview_request_or_autostart() {
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-preview-{}",
        uuid::Uuid::new_v4()
    );
    let mut pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
    let server = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        teamy_broker::protocol::wire::write(
            &mut pipe,
            &teamy_broker::protocol::Hello {
                version: 2,
                capabilities: teamy_broker::protocol::CAPABILITIES
                    .iter()
                    .map(|cap| (*cap).into())
                    .collect(),
                instance_id: String::new(),
            },
        )
        .await
        .unwrap();
        let _ = teamy_broker::protocol::wire::read::<teamy_broker::protocol::Hello, _>(&mut pipe)
            .await
            .unwrap_err();
    });
    let error = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3))
        .approval_preview()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "broker approval preview unsupported; update broker explicitly"
    );
    server.await.unwrap();
}
#[test]
fn preview_shape_cannot_supply_a_reference_or_lease_and_cli_requires_explicit_ui_flag() {
    use teamy_broker::protocol::Request;
    assert_eq!(teamy_broker::protocol::VERSION, 2);
    Request::control("approval-preview", None)
        .validate()
        .unwrap();
    let _ = Request::control("approval-preview", Some(uuid::Uuid::new_v4().to_string()))
        .validate()
        .unwrap_err();
    let mut invalid = Request::control("approval-preview", None);
    invalid.acquire = Some(teamy_broker::protocol::AcquireRequest {
        account: "dummy".into(),
        secret_ref: "op://Synthetic/not-used/field".into(),
        purpose: "not-used".into(),
        requesting_app: "not-used".into(),
        destination: "https://example.invalid".into(),
        backend: teamy_broker::protocol::Backend::Memory,
    });
    let _ = invalid.validate().unwrap_err();
    for (args, expected) in [
        (vec!["preview"], false),
        (vec!["preview", "--show-test-dialog"], true),
    ] {
        let cli: teamy_broker::cli::Cli = figue::Driver::new(
            figue::builder::<teamy_broker::cli::Cli>()
                .unwrap()
                .cli(|cli| cli.args(args).strict())
                .build(),
        )
        .run()
        .into_result()
        .unwrap()
        .value;
        assert!(
            matches!(cli.command.into_request().unwrap(),teamy_broker::broker::app_request::AppRequest::Preview(show) if show==expected)
        );
    }
}
