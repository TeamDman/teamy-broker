mod support;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::server::serve_at;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::FailureKind;
use teamy_broker::sdk::Client;
use teamy_cancellation::CancellationToken;

#[tokio::test]
async fn ipc_status_stays_responsive_and_guarded_stop_rechecks_current_work() {
    let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
    let release = Arc::new(tokio::sync::Notify::new());
    let approval = Arc::new(FakeApproval {
        dialog: true,
        release: Some(Arc::<tokio::sync::Notify>::clone(&release)),
        ..Default::default()
    });
    let engine = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::<FakeApproval>::clone(&approval),
        Arc::new(FakeStorage::default()),
    ));
    let token = CancellationToken::new();
    let server = tokio::spawn({
        let endpoint = endpoint.clone();
        async move { serve_at(engine, token, &endpoint, false).await }
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    while !client.status().await.unwrap().running {
        tokio::task::yield_now().await;
    }
    assert!(client.status().await.unwrap().safe_to_stop);
    let acquire = tokio::spawn({
        let client = client.clone();
        async move { client.acquire(support::request(Backend::Memory)).await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while approval.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let busy = tokio::time::timeout(Duration::from_millis(200), client.status())
        .await
        .unwrap()
        .unwrap();
    assert!(busy.shutdown_state_available);
    assert_eq!(busy.open_approval_dialogs, 1);
    assert_eq!(busy.active_requests, 1);
    assert_eq!(
        client
            .stop()
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
    assert!(client.status().await.unwrap().running);
    release.notify_one();
    let secret = acquire.await.unwrap().unwrap();
    assert_eq!(client.status().await.unwrap().active_leases, 1);
    drop(secret);
    assert_eq!(
        client
            .stop()
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
    client.stop_force().await.unwrap();
    server.await.unwrap().unwrap();
    let stopped = client.status().await.unwrap();
    assert!(!stopped.running);
    assert!(stopped.safe_to_stop);
}

#[test]
fn legacy_status_defaults_to_unknown_and_protocol_compatibility_is_preserved() {
    let status: teamy_broker::protocol::Status =
        facet_json::from_str(r#"{"running":true,"active_leases":0,"cleanup_failures":0}"#).unwrap();
    assert!(!status.shutdown_state_available);
    assert!(!status.safe_to_stop);
    assert_eq!(teamy_broker::protocol::VERSION, 2);
    let hello = teamy_broker::protocol::Hello {
        version: 2,
        capabilities: teamy_broker::protocol::CAPABILITIES
            .iter()
            .map(|cap| (*cap).into())
            .collect(),
        instance_id: String::new(),
    };
    assert!(hello.compatible());
}

#[tokio::test]
async fn safe_stop_never_sends_shutdown_to_a_legacy_server() {
    let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
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
        assert!(
            teamy_broker::protocol::wire::read::<teamy_broker::protocol::Hello, _>(&mut pipe)
                .await
                .is_err(),
            "unsupported safe-stop client must close before writing any request"
        );
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    let error = client.stop().await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "broker shutdown status unsupported; update broker explicitly"
    );
    server.await.unwrap();
}
