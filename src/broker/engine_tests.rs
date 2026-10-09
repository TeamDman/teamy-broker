use super::*;
#[path = "../../tests/support/mod.rs"]
mod support;
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
