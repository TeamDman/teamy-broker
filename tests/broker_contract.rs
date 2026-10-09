mod support;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::lease::Lease;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::Hello;
use teamy_broker::protocol::Request;
use teamy_broker::protocol::{self};
use teamy_broker::sdk::resolve_provider;
fn engine(
    adapter: Arc<FakeAdapter>,
    approval: Arc<FakeApproval>,
    storage: Arc<FakeStorage>,
) -> Arc<Engine> {
    Arc::new(Engine::new(adapter, approval, storage))
}
#[tokio::test]
async fn multiple_clients_share_only_exact_approved_use_and_fixed_deadline() {
    let adapter = Arc::new(FakeAdapter::default());
    let approval = Arc::new(FakeApproval::default());
    let storage = Arc::new(FakeStorage::default());
    let e = engine(Arc::clone(&adapter), Arc::clone(&approval), storage);
    let mut clients = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let e = Arc::clone(&e);
        clients.spawn(async move {
            e.acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap()
        });
    }
    let first = clients.join_next().await.unwrap().unwrap();
    while let Some(result) = clients.join_next().await {
        let next = result.unwrap();
        assert_eq!(next.id, first.id);
        assert_eq!(next.expires, first.expires);
    }
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
    assert_eq!(approval.calls.load(Ordering::SeqCst), 1);
    let mut changed = request(Backend::Memory);
    changed.destination = "https://other.example.invalid".into();
    let other = e
        .acquire(changed, 2, Arc::new(AtomicBool::new(false)))
        .await
        .unwrap();
    assert_ne!(first.id, other.id);
    assert_eq!(approval.calls.load(Ordering::SeqCst), 2);
    e.purge(Some(&first.id)).await.unwrap();
    assert_eq!(e.status().await.unwrap().active_leases, 1);
    e.purge(None).await.unwrap();
    assert_eq!(e.status().await.unwrap().active_leases, 0);
}
#[tokio::test]
async fn denied_approval_never_reads_and_adapter_denial_never_publishes() {
    let a = Arc::new(FakeAdapter::default());
    let s = Arc::new(FakeStorage::default());
    let e = engine(
        Arc::clone(&a),
        Arc::new(FakeApproval {
            denied: true,
            ..Default::default()
        }),
        Arc::clone(&s),
    );
    let _ = e
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
    assert!(s.published.lock().unwrap().is_empty());
    let e = engine(
        Arc::new(FakeAdapter {
            fail: true,
            ..Default::default()
        }),
        Arc::new(FakeApproval::default()),
        Arc::clone(&s),
    );
    let _ = e
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
    assert!(s.published.lock().unwrap().is_empty());
}
#[tokio::test]
async fn registration_failure_never_grants_and_cleanup_failure_removes_visible_lease() {
    let s = Arc::new(FakeStorage::default());
    s.fail_publish.store(true, Ordering::SeqCst);
    let e = engine(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::clone(&s),
    );
    let _ = e
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
    assert_eq!(e.status().await.unwrap().active_leases, 0);
    s.fail_publish.store(false, Ordering::SeqCst);
    let granted = e
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    s.fail_remove.store(true, Ordering::SeqCst);
    let _ = e.purge(Some(&granted.id)).await;
    let status = e.status().await.unwrap();
    assert_eq!(status.active_leases, 0);
    assert!(status.cleanup_failures > 0);
    s.fail_remove.store(false, Ordering::SeqCst);
    e.purge(None).await.unwrap();
}
#[tokio::test]
async fn purge_during_approval_cancels_publication() {
    let a = Arc::new(FakeAdapter::default());
    let approval = Arc::new(FakeApproval {
        delay: Duration::from_millis(100),
        ..Default::default()
    });
    let s = Arc::new(FakeStorage::default());
    let e = engine(Arc::clone(&a), Arc::clone(&approval), Arc::clone(&s));
    let task = {
        let e = Arc::clone(&e);
        tokio::spawn(async move {
            e.acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false)),
            )
            .await
        })
    };
    while approval.calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    e.purge(None).await.unwrap();
    let _ = task.await.unwrap().unwrap_err();
    assert!(s.published.lock().unwrap().is_empty());
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn all_explicit_backends_obey_same_approval_and_purge_contract() {
    for backend in [
        Backend::Memory,
        Backend::WindowsCredentialManager("teamy-broker/synthetic-test".into()),
        Backend::WellKnownFile(
            std::env::temp_dir()
                .join("teamy-broker-synthetic-test.bin")
                .to_string_lossy()
                .into_owned(),
        ),
    ] {
        let s = Arc::new(FakeStorage::default());
        let e = engine(
            Arc::new(FakeAdapter::default()),
            Arc::new(FakeApproval::default()),
            Arc::clone(&s),
        );
        let lease = e
            .acquire(request(backend), 1, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_eq!(s.published.lock().unwrap().len(), 1);
        e.purge(Some(&lease.id)).await.unwrap();
        assert_eq!(s.removals.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn lease_is_exactly_fifteen_minutes_and_boundary_rejects_even_before_cleanup() {
    let now = protocol::wire::unix_ms().unwrap();
    let mut lease = Lease::new(
        request(Backend::Memory),
        zeroize::Zeroizing::new(b"synthetic".to_vec()),
        now,
        Instant::now(),
    )
    .unwrap();
    assert_eq!(lease.expires - now, 900_000);
    assert!(lease.valid(lease.expires - 1));
    assert!(!lease.valid(lease.expires));
    lease.deadline = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    assert!(!lease.valid(now));
}
#[test]
fn provider_conflicts_and_invalid_explicit_choices_never_fallback() {
    let _ = resolve_provider(Some("teamy-broker"), Some("teamy-broker")).unwrap_err();
    let _ = resolve_provider(None, None).unwrap_err();
    let _ = resolve_provider(Some("invalid"), None).unwrap_err();
    for name in [
        "teamy-broker",
        "windows-credential-manager",
        "well-known-file",
    ] {
        assert_eq!(resolve_provider(Some(name), None).unwrap(), name);
        assert_eq!(resolve_provider(None, Some(name)).unwrap(), name);
    }
}
#[test]
fn protocol_shape_compatibility_and_redaction() {
    let mut hello = Hello {
        version: protocol::VERSION,
        capabilities: protocol::CAPABILITIES.iter().map(|c| (*c).into()).collect(),
        instance_id: "synthetic".into(),
    };
    assert!(hello.compatible());
    hello.version += 1;
    assert!(!hello.compatible());
    hello.version = protocol::VERSION;
    hello.capabilities.pop();
    assert!(!hello.compatible());
    let request = Request {
        operation: "execute-arbitrary-command".into(),
        acquire: None,
        lease_id: None,
    };
    assert!(request.validate().is_err());
    assert!(!format!("{:?}", support::request(Backend::Memory)).contains("Synthetic"));
    let r = support::request(Backend::Memory);
    let text = facet_json::to_string(&r).unwrap();
    let roundtrip: protocol::AcquireRequest = facet_json::from_str(&text).unwrap();
    assert_eq!(r, roundtrip);
}
