use super::*;
#[path = "../../tests/support/mod.rs"]
pub(crate) mod support;
use crate::protocol::Backend;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
#[tokio::test]
async fn delayed_cleanup_cannot_keep_expired_secret_retrievable_and_retries_clear_it() {
    let storage = Arc::new(FakeStorage::default());
    let engine = Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::<FakeStorage>::clone(&storage),
    );
    let grant = engine
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    storage.fail_remove.store(true, Ordering::SeqCst);
    engine
        .leases
        .lock()
        .await
        .get_mut(&grant.id)
        .unwrap()
        .expires = crate::protocol::wire::unix_ms().unwrap();
    let _ = engine.validate_lease(&grant.id).await.unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
    assert!(
        engine
            .pending
            .lock()
            .await
            .iter()
            .all(|lease| lease.secret.is_empty())
    );
    storage.fail_remove.store(false, Ordering::SeqCst);
    engine.purge(None).await.unwrap();
    assert!(engine.pending.lock().await.is_empty());
}
#[tokio::test]
async fn monotonic_expiry_rejects_when_wall_clock_deadline_is_still_future() {
    let engine = Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::new(FakeStorage::default()),
    );
    let grant = engine
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    engine
        .leases
        .lock()
        .await
        .get_mut(&grant.id)
        .unwrap()
        .deadline = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();
    let _ = engine.validate_lease(&grant.id).await.unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}
#[tokio::test]
async fn shutdown_during_adapter_read_prevents_publication() {
    let adapter = Arc::new(FakeAdapter {
        delay: std::time::Duration::from_millis(100),
        ..Default::default()
    });
    let storage = Arc::new(FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::new(FakeApproval::default()),
        Arc::<FakeStorage>::clone(&storage),
    ));
    let operation = {
        let engine = Arc::<Engine>::clone(&engine);
        tokio::spawn(async move {
            engine
                .acquire(
                    request(Backend::Memory),
                    1,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
        })
    };
    while adapter.calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    engine.shutdown().await.unwrap();
    let _ = operation.await.unwrap().unwrap_err();
    assert!(storage.published.lock().unwrap().is_empty());
}

#[path = "../../tests/support/shared_approval.rs"]
pub(crate) mod shared_approval;
#[tokio::test]
async fn phone_supply_enters_normal_memory_lease_without_provider_read() {
    let request = request(Backend::Memory);
    let cancel = Arc::new(AtomicBool::new(false));
    let hub = crate::broker::shared_preview::SharedPreviewHub::default();
    let shared = hub
        .begin_credential(
            Arc::clone(&cancel),
            crate::protocol::credential_context::CredentialContext {
                request: request.clone(),
                supply: true,
                connected_process_id: 1,
                provider_state: "idle".into(),
                synthetic_fixture: false,
            },
        )
        .unwrap();
    shared.supply(zeroize::Zeroizing::new(b"synthetic-memory-value".to_vec()));
    let adapter = Arc::new(FakeAdapter::default());
    let storage = Arc::new(FakeStorage::default());
    let engine = Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::new(shared_approval::SharedApproval(shared)),
        Arc::<FakeStorage>::clone(&storage),
    );
    let grant = engine.acquire(request, 1, cancel).await.unwrap();
    assert_eq!(&*grant.bytes, b"synthetic-memory-value");
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert_eq!(storage.published.lock().unwrap().len(), 1);
    engine.validate_lease(&grant.id).await.unwrap();
    assert_eq!(engine.status().await.unwrap().active_leases, 1);
    assert_eq!(engine.status().await.unwrap().upstream_reads, 0);
    engine.purge(None).await.unwrap();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}
#[tokio::test]
async fn shared_denial_does_not_publish_or_read() {
    let request = request(Backend::Memory);
    let cancel = Arc::new(AtomicBool::new(false));
    let hub = crate::broker::shared_preview::SharedPreviewHub::default();
    let shared = hub
        .begin_credential(
            Arc::clone(&cancel),
            crate::protocol::credential_context::CredentialContext {
                request: request.clone(),
                supply: true,
                connected_process_id: 1,
                provider_state: "idle".into(),
                synthetic_fixture: false,
            },
        )
        .unwrap();
    shared.resolve(crate::protocol::ApprovalPreviewDisposition::DeniedTest);
    let engine = Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(shared_approval::SharedApproval(shared)),
        Arc::new(FakeStorage::default()),
    );
    assert_eq!(
        engine
            .acquire(request, 1, cancel)
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::UserDenied)
    );
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}
