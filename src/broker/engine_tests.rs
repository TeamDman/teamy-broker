use super::*;
use crate::broker::shared_preview::SharedPreviewHub;
use crate::protocol::credential_context::CredentialContext;
#[path = "../../tests/support/mod.rs"]
pub(crate) mod support;
use crate::protocol::Backend;
use std::thread;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
use tokio::runtime::Builder;
#[path = "engine_tests/blocked_storage.rs"]
mod blocked_storage;
#[path = "engine_tests/publication_delivery.rs"]
mod publication_delivery;
#[path = "engine_tests/server_responsiveness.rs"]
mod server_responsiveness;
use blocked_storage::BlockedStorage;

fn blocked_engine() -> (Arc<Engine>, Arc<BlockedStorage>) {
    let storage = Arc::new(BlockedStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::<BlockedStorage>::clone(&storage),
    ));
    (engine, storage)
}

fn acquire_blocked(engine: &Arc<Engine>) -> task::JoinHandle<eyre::Result<Grant>> {
    let engine = Arc::clone(engine);
    tokio::spawn(async move {
        engine
            .acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    })
}

#[tokio::test]
async fn abandoned_publication_retains_work_and_tracks_failed_rollback_without_secrets() {
    let (engine, storage) = blocked_engine();
    let acquisition = acquire_blocked(&engine);
    time::timeout(Duration::from_secs(2), storage.started.notified())
        .await
        .unwrap();
    acquisition.abort();
    assert!(acquisition.await.unwrap_err().is_cancelled());
    let status = time::timeout(Duration::from_millis(200), engine.status())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.active_requests, 1);
    assert_eq!(status.storage_operations, 1);
    assert_eq!(status.active_leases, 0);
    storage.fail_remove.store(true, Ordering::SeqCst);
    storage.release();
    time::timeout(Duration::from_secs(2), engine.storage.tasks.drain())
        .await
        .unwrap();
    let status = engine.status().await.unwrap();
    assert_eq!(status.storage_operations, 0);
    assert_eq!(status.active_requests, 0);
    assert_eq!(status.active_leases, 0);
    assert_eq!(status.pending_cleanup, 1);
    assert_eq!(status.cleanup_failures, 1);
    assert_eq!(storage.published.lock().unwrap().len(), 1);
    assert_eq!(storage.removed.lock().unwrap().len(), 1);
    assert!(
        engine
            .storage
            .pending
            .lock()
            .await
            .iter()
            .all(|lease| lease.secret.is_empty())
    );
    storage.fail_remove.store(false, Ordering::SeqCst);
    engine.purge(None).await.unwrap();
    assert!(engine.status().await.unwrap().safe_to_stop);
}

#[tokio::test]
async fn purge_and_forced_shutdown_drain_native_publication_before_returning() {
    for shutdown in [false, true] {
        let (engine, storage) = blocked_engine();
        let acquisition = acquire_blocked(&engine);
        time::timeout(Duration::from_secs(2), storage.started.notified())
            .await
            .unwrap();
        let controlling = Arc::clone(&engine);
        let control = tokio::spawn(async move {
            if shutdown {
                controlling.stop(true).await
            } else {
                controlling.purge(None).await
            }
        });
        time::timeout(Duration::from_secs(2), async {
            while engine.storage.epoch.load(Ordering::SeqCst) == 0 {
                task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!control.is_finished());
        let status = time::timeout(Duration::from_millis(200), engine.status())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.stopping, shutdown);
        assert_eq!(status.active_leases, 0);
        assert!(status.storage_operations > 0);
        // Already admitted native IO finishes physically; invalidation prevents delivery
        // and the control operation waits for the matching rollback before reporting success.
        storage.release();
        let _ = acquisition.await.unwrap().unwrap_err();
        time::timeout(Duration::from_secs(2), control)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(storage.published.lock().unwrap().len(), 1);
        assert_eq!(storage.removed.lock().unwrap().len(), 1);
        let status = engine.status().await.unwrap();
        assert_eq!(status.active_leases, 0);
        assert_eq!(status.pending_cleanup, 0);
        assert_eq!(status.storage_operations, 0);
    }
}

#[tokio::test]
async fn canceled_publication_keeps_the_serial_gate_until_rollback_finishes() {
    let (engine, storage) = blocked_engine();
    let first = acquire_blocked(&engine);
    time::timeout(Duration::from_secs(2), storage.started.notified())
        .await
        .unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    let second = acquire_blocked(&engine);
    time::timeout(Duration::from_secs(2), async {
        while engine.status().await.unwrap().queued_requests != 1 {
            task::yield_now().await;
        }
    })
    .await
    .unwrap();
    storage.release();
    let grant = time::timeout(Duration::from_secs(2), second)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let published = storage.published.lock().unwrap().clone();
    assert_eq!(published.len(), 2);
    assert_ne!(published[0], grant.id);
    assert_eq!(published[1], grant.id);
    assert_eq!(*storage.removed.lock().unwrap(), [published[0].clone()]);
    engine.validate_lease(&grant.id).await.unwrap();
    engine.purge(None).await.unwrap();
}

#[tokio::test]
async fn canceled_purge_retains_the_entire_removed_batch_until_cleanup_finishes() {
    let (engine, storage) = blocked_engine();
    storage.release();
    for suffix in ["first", "second"] {
        let mut request = request(Backend::Memory);
        request.destination = format!("https://example.invalid/{suffix}");
        drop(
            engine
                .acquire(request, 1, Arc::new(AtomicBool::new(false)))
                .await
                .unwrap(),
        );
    }
    storage.block_remove.store(true, Ordering::SeqCst);
    let purging = Arc::clone(&engine);
    let purge = tokio::spawn(async move { purging.purge(None).await });
    time::timeout(Duration::from_secs(2), storage.remove_started.notified())
        .await
        .unwrap();
    let status = time::timeout(Duration::from_millis(200), engine.status())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.active_leases, 0);
    assert!(status.storage_operations > 0);
    purge.abort();
    assert!(purge.await.unwrap_err().is_cancelled());
    storage.release_removal();
    time::timeout(Duration::from_secs(2), engine.storage.tasks.drain())
        .await
        .unwrap();
    assert_eq!(storage.removed.lock().unwrap().len(), 2);
    assert!(engine.status().await.unwrap().safe_to_stop);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_and_ordinary_stop_respond_while_storage_publication_is_blocked() {
    let storage = Arc::new(BlockedStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::<BlockedStorage>::clone(&storage),
    ));
    let running = Arc::clone(&engine);
    let acquisition = tokio::spawn(async move {
        running
            .acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    });
    // A separate runtime keeps the probe's deadline independent of the blocked worker.
    let probe_engine = Arc::clone(&engine);
    let probe_storage = Arc::clone(&storage);
    let probe = thread::spawn(move || {
        let result = Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                time::timeout(Duration::from_secs(2), probe_storage.started.notified())
                    .await
                    .unwrap();
                tokio::join!(
                    time::timeout(Duration::from_millis(200), probe_engine.status()),
                    time::timeout(Duration::from_millis(200), probe_engine.stop(false)),
                )
            });
        // Release before asserting so the failing baseline cannot leave a blocked worker.
        probe_storage.release();
        result
    });
    let grant = acquisition.await.unwrap().unwrap();
    let (status, stop) = probe.join().unwrap();
    engine.purge(Some(&grant.id)).await.unwrap();
    let status = status
        .expect("status must not wait for native publication")
        .unwrap();
    assert_eq!(status.active_leases, 0);
    assert_eq!(status.storage_operations, 1);
    assert!(!status.safe_to_stop);
    assert_eq!(
        stop.expect("ordinary stop must promptly reject active storage")
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
}

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
        .expires = wire::unix_ms().unwrap();
    let _ = engine.validate_lease(&grant.id).await.unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
    assert!(
        engine
            .storage
            .pending
            .lock()
            .await
            .iter()
            .all(|lease| lease.secret.is_empty())
    );
    storage.fail_remove.store(false, Ordering::SeqCst);
    engine.purge(None).await.unwrap();
    assert!(engine.storage.pending.lock().await.is_empty());
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
        .deadline = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    let _ = engine.validate_lease(&grant.id).await.unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}
#[tokio::test]
async fn shutdown_during_adapter_read_prevents_publication() {
    let adapter = Arc::new(FakeAdapter {
        delay: Duration::from_millis(100),
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
        task::yield_now().await;
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
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(
            Arc::clone(&cancel),
            CredentialContext {
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
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(
            Arc::clone(&cancel),
            CredentialContext {
                request: request.clone(),
                supply: true,
                connected_process_id: 1,
                provider_state: "idle".into(),
                synthetic_fixture: false,
            },
        )
        .unwrap();
    shared.resolve(ApprovalPreviewDisposition::DeniedTest);
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
