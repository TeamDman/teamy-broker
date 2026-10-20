mod support;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::server::serve_at;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::CAPABILITIES;
use teamy_broker::protocol::Hello;
use teamy_broker::protocol::VERSION;
use teamy_broker::sdk::Client;
use teamy_cancellation::CancellationToken;
fn test_endpoint() -> String {
    format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4())
}
#[tokio::test]
async fn real_windows_pipe_multiple_clients_purge_stop_and_single_instance() {
    let endpoint = test_endpoint();
    let approval = Arc::new(FakeApproval::default());
    let e = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::<FakeApproval>::clone(&approval),
        Arc::new(FakeStorage::default()),
    ));
    let cancel = CancellationToken::new();
    let server = {
        let e = Arc::<Engine>::clone(&e);
        let cancel = cancel.clone();
        let endpoint = endpoint.clone();
        tokio::spawn(async move { serve_at(e, cancel, &endpoint, false).await })
    };
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint.clone())
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    while !client.status().await.unwrap().running {
        tokio::task::yield_now().await;
    }
    assert!(
        serve_at(e, CancellationToken::new(), &endpoint, false)
            .await
            .is_err()
    );
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let client = client.clone();
        tasks.spawn(async move { client.acquire(request(Backend::Memory)).await.unwrap() });
    }
    let first = tasks.join_next().await.unwrap().unwrap();
    while let Some(next) = tasks.join_next().await {
        assert_eq!(first.lease_id, next.unwrap().lease_id);
    }
    assert_eq!(approval.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        first.expose(<[u8]>::len).unwrap(),
        b"SYNTHETIC_SECRET_NEVER_LOG".len()
    );
    assert!(!format!("{first:?}").contains("SYNTHETIC"));
    assert_eq!(client.status().await.unwrap().active_leases, 1);
    client.purge(None).await.unwrap();
    assert_eq!(client.status().await.unwrap().active_leases, 0);
    client.stop_force().await.unwrap();
    server.await.unwrap().unwrap();
    assert!(!client.status().await.unwrap().running);
}
#[tokio::test]
async fn incompatible_peer_does_not_autostart_and_stalled_peer_times_out() {
    let endpoint = test_endpoint();
    let mut pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
    let peer = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        teamy_broker::protocol::wire::write(
            &mut pipe,
            &Hello {
                version: VERSION + 1,
                capabilities: CAPABILITIES.iter().map(|c| (*c).into()).collect(),
                instance_id: String::new(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    let client = Client::new(Some("this-must-never-be-started.exe".into()))
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_millis(200), Duration::from_millis(300));
    let error = client.acquire(request(Backend::Memory)).await.unwrap_err();
    assert!(error.to_string().contains("incompatible"));
    peer.await.unwrap();
    let endpoint = test_endpoint();
    let pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
    let peer = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_millis(25), Duration::from_millis(100));
    let _ = client.acquire(request(Backend::Memory)).await.unwrap_err();
    peer.await.unwrap();
}
#[tokio::test]
async fn client_timeout_cancels_approval_and_prevents_adapter_read() {
    let endpoint = test_endpoint();
    let adapter = Arc::new(FakeAdapter::default());
    let approval = Arc::new(FakeApproval {
        delay: Duration::from_millis(150),
        ..Default::default()
    });
    let e = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        approval,
        Arc::new(FakeStorage::default()),
    ));
    let cancel = CancellationToken::new();
    let server = {
        let cancel = cancel.clone();
        let endpoint = endpoint.clone();
        tokio::spawn(async move { serve_at(e, cancel, &endpoint, false).await })
    };
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(1), Duration::from_millis(50));
    while !client.status().await.unwrap().running {
        tokio::task::yield_now().await;
    }
    let _ = client.acquire(request(Backend::Memory)).await.unwrap_err();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    cancel.request_cancel("synthetic test complete");
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn sdk_autostart_race_has_one_ready_daemon_and_no_orphan_after_stop() {
    let endpoint = test_endpoint();
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .parent()
        .unwrap()
        .join("examples")
        .join("synthetic_broker.exe");
    assert!(
        executable.is_file(),
        "build synthetic_broker example before running integration tests"
    );
    let client = Client::new(Some(executable))
        .unwrap()
        .with_endpoint(endpoint.clone())
        .with_start_arguments(vec![
            "--synthetic-test-only".into(),
            "--endpoint".into(),
            endpoint,
        ])
        .with_deadlines(Duration::from_secs(5), Duration::from_secs(10));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let client = client.clone();
        tasks.spawn(async move { client.acquire(request(Backend::Memory)).await.unwrap() });
    }
    let first = tasks.join_next().await.unwrap().unwrap();
    while let Some(result) = tasks.join_next().await {
        assert_eq!(first.lease_id, result.unwrap().lease_id);
    }
    client.stop_force().await.unwrap();
    for _ in 0..100 {
        if !client.status().await.unwrap().running {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("synthetic daemon did not stop");
}
