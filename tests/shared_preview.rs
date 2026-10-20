#[path = "support/fake_adapter.rs"]
mod fake_adapter;
#[path = "support/fake_preview_approval.rs"]
mod fake_preview_approval;
#[path = "support/fake_storage.rs"]
mod fake_storage;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::shared_preview::SharedPreviewHub;
use teamy_broker::protocol::ApprovalPreviewDisposition;
use teamy_broker::protocol::Request;
use teamy_broker::sdk::Client;
#[tokio::test]
async fn resident_ipc_reconciles_shared_decisions_without_provider_storage_or_lease() {
    let adapter = Arc::new(fake_adapter::FakeAdapter::default());
    let storage = Arc::new(fake_storage::FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<fake_adapter::FakeAdapter>::clone(&adapter),
        Arc::new(fake_preview_approval::FakePreviewApproval::new(
            ApprovalPreviewDisposition::DeniedTest,
        )),
        Arc::<fake_storage::FakeStorage>::clone(&storage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-shared-preview-test-{}",
        uuid::Uuid::new_v4()
    );
    let server = tokio::spawn({
        let engine = Arc::clone(&engine);
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
        loop {
            if client.shared_preview_state().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let hub = SharedPreviewHub::resident();
    let cancel = Arc::new(AtomicBool::new(false));
    let request = hub.begin(Arc::clone(&cancel)).unwrap();
    let id = client
        .shared_preview_state()
        .await
        .unwrap()
        .pending
        .unwrap()
        .request_id;
    let desktop = client.clone();
    let phone = client.clone();
    let (a, b) = tokio::join!(
        desktop.shared_preview_decide(&id, ApprovalPreviewDisposition::ApprovedTest),
        phone.shared_preview_decide(&id, ApprovalPreviewDisposition::DeniedTest)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    assert!(request.outcome().is_some());
    assert!(
        client
            .shared_preview_state()
            .await
            .unwrap()
            .pending
            .is_none()
    );
    assert!(
        !client
            .shared_preview_decide(&id, ApprovalPreviewDisposition::ApprovedTest)
            .await
            .unwrap()
    );
    cancellation_reconciles(&client, hub, &cancel, &id).await;
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(storage.removals.load(Ordering::SeqCst), 0);
    assert_eq!(client.status().await.unwrap().active_leases, 0);
    client.stop().await.unwrap();
    server.await.unwrap().unwrap();
}
#[test]
fn malformed_preview_commands_cannot_inject_acquisition() {
    for operation in [
        "preview-state",
        "preview-approve",
        "preview-deny",
        "preview-cancel",
    ] {
        let mut request = Request::control(operation, Some("not-a-preview".into()));
        assert!(request.validate().is_err());
        request.acquire = Some(teamy_broker::broker::mobile_dummy::request());
        assert!(request.validate().is_err());
    }
}

async fn cancellation_reconciles(
    client: &Client,
    hub: &SharedPreviewHub,
    cancel: &Arc<AtomicBool>,
    id: &str,
) {
    let request = hub.begin(Arc::clone(cancel)).unwrap();
    let next = client
        .shared_preview_state()
        .await
        .unwrap()
        .pending
        .unwrap()
        .request_id;
    assert_ne!(id, next);
    cancel.store(true, Ordering::SeqCst);
    assert!(
        client
            .shared_preview_state()
            .await
            .unwrap()
            .pending
            .is_none()
    );
    assert!(
        !client
            .shared_preview_decide(&next, ApprovalPreviewDisposition::ApprovedTest)
            .await
            .unwrap()
    );
    assert_eq!(
        request.outcome(),
        Some(ApprovalPreviewDisposition::Cancelled)
    );
}
