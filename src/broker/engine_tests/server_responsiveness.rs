use super::*;
use crate::broker::server::serve_at;
use crate::sdk::Client;
use teamy_cancellation::CancellationToken;
use uuid::Uuid;

#[tokio::test]
async fn new_ipc_status_clients_are_served_while_expiry_cleanup_is_blocked() {
    let (engine, storage) = blocked_engine();
    storage.release();
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
        .deadline = Instant::now();
    drop(grant);
    storage.block_remove.store(true, Ordering::SeqCst);
    let endpoint = format!(r"\\.\pipe\teamy-broker-maintenance-{}", Uuid::new_v4());
    let token = CancellationToken::new();
    let serving = Arc::clone(&engine);
    let server_endpoint = endpoint.clone();
    let server_token = token.clone();
    let server =
        tokio::spawn(async move { serve_at(serving, server_token, &server_endpoint, false).await });
    time::timeout(Duration::from_secs(2), storage.remove_started.notified())
        .await
        .unwrap();
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(2));
    let status = time::timeout(Duration::from_millis(200), client.status()).await;
    token.request_cancel("isolated maintenance regression complete");
    // Shutdown must drain the blocked native cleanup before releasing its listener.
    time::sleep(Duration::from_millis(150)).await;
    assert!(!server.is_finished());
    storage.release_removal();
    time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let status = status
        .expect("new IPC status must not wait for expiry cleanup")
        .unwrap();
    assert!(status.running);
    assert_eq!(status.active_leases, 0);
    assert!(status.storage_operations > 0);
    assert_eq!(storage.removed.lock().unwrap().len(), 1);
    assert_eq!(engine.status().await.unwrap().storage_operations, 0);
}
