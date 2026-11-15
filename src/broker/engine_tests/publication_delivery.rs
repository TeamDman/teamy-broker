use super::super::lease_delivery::LeaseDelivery;
use super::*;

async fn buffered_delivery(engine: &Arc<Engine>, cancel: Arc<AtomicBool>) -> LeaseDelivery {
    let work = engine.activity.request().unwrap();
    let lease = Lease::new(
        request(Backend::Memory),
        zeroize::Zeroizing::new(b"synthetic-publication".to_vec()),
        wire::unix_ms().unwrap(),
        Instant::now(),
    )
    .unwrap();
    let (finished, completion) = oneshot::channel();
    let publication = LeasePublication {
        state: Arc::clone(&engine.storage),
        lease: Some(lease),
        _work: work.phase(WorkKind::Storage),
        request_work: Some(work),
        cancel,
        abandoned: Arc::new(AtomicBool::new(false)),
        epoch: engine.storage.epoch.load(Ordering::SeqCst),
        provenance: None,
        _gate: Arc::clone(&engine.gate).lock_owned().await,
        _tracked: engine.storage.tasks.begin(),
    };
    task::spawn_blocking(move || publication.run(finished));
    time::timeout(Duration::from_secs(2), completion)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn unacknowledged_delivery_rolls_back_only_its_own_lease() {
    let (engine, storage) = blocked_engine();
    storage.release();
    let unrelated = Lease::new(
        request(Backend::Memory),
        zeroize::Zeroizing::new(b"unrelated-generation".to_vec()),
        wire::unix_ms().unwrap(),
        Instant::now(),
    )
    .unwrap();
    let unrelated_id = unrelated.id.clone();
    engine
        .leases
        .lock()
        .await
        .insert(unrelated_id.clone(), unrelated);
    let delivery = buffered_delivery(&engine, Arc::new(AtomicBool::new(false))).await;
    let abandoned_id = delivery.grant.id.clone();
    assert!(engine.leases.lock().await.contains_key(&abandoned_id));
    drop(delivery);
    time::timeout(Duration::from_secs(2), engine.storage.tasks.drain())
        .await
        .unwrap();
    let leases = engine.leases.lock().await;
    assert!(!leases.contains_key(&abandoned_id));
    assert!(leases.contains_key(&unrelated_id));
    drop(leases);
    assert_eq!(*storage.removed.lock().unwrap(), [abandoned_id]);
    engine.purge(None).await.unwrap();
}

#[tokio::test]
async fn shutdown_drains_while_a_caller_retains_an_unreceived_delivery() {
    let (engine, storage) = blocked_engine();
    storage.release();
    let delivery = buffered_delivery(&engine, Arc::new(AtomicBool::new(false))).await;
    let id = delivery.grant.id.clone();
    time::timeout(Duration::from_secs(2), engine.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*storage.removed.lock().unwrap(), [id]);
    assert_eq!(engine.status().await.unwrap().storage_operations, 0);
    let _ = delivery.accept().await.unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_requests, 0);
}

#[tokio::test]
async fn buffered_delivery_rechecks_cancellation_and_expiry_before_handoff() {
    for cancel_before_handoff in [false, true] {
        let (engine, storage) = blocked_engine();
        storage.release();
        let cancel = Arc::new(AtomicBool::new(false));
        let delivery = buffered_delivery(&engine, Arc::clone(&cancel)).await;
        let id = delivery.grant.id.clone();
        if cancel_before_handoff {
            cancel.store(true, Ordering::SeqCst);
        } else {
            engine.leases.lock().await.get_mut(&id).unwrap().deadline = Instant::now();
        }
        let _ = delivery.accept().await.unwrap_err();
        time::timeout(Duration::from_secs(2), engine.storage.tasks.drain())
            .await
            .unwrap();
        assert_eq!(*storage.removed.lock().unwrap(), [id]);
        assert_eq!(engine.status().await.unwrap().active_leases, 0);
    }
}

#[tokio::test]
async fn expired_shared_approval_rolls_back_an_already_admitted_native_write() {
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
    assert!(shared.supply(zeroize::Zeroizing::new(b"synthetic-memory-value".to_vec())));
    let storage = Arc::new(BlockedStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(shared_approval::SharedApproval(Arc::clone(&shared))),
        Arc::<BlockedStorage>::clone(&storage),
    ));
    let running = Arc::clone(&engine);
    let acquisition = tokio::spawn(async move { running.acquire(request, 1, cancel).await });
    time::timeout(Duration::from_secs(2), storage.started.notified())
        .await
        .unwrap();
    shared.expire_original_deadline();
    storage.release();
    assert_eq!(
        acquisition
            .await
            .unwrap()
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ApprovalTimedOut)
    );
    time::timeout(Duration::from_secs(2), engine.storage.tasks.drain())
        .await
        .unwrap();
    assert_eq!(storage.published.lock().unwrap().len(), 1);
    assert_eq!(storage.removed.lock().unwrap().len(), 1);
    assert!(engine.status().await.unwrap().safe_to_stop);
}
