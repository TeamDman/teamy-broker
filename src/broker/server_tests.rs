use super::*;
use crate::broker::engine::tests::support;
use crate::protocol::Backend;
use crate::sdk::Client;
use std::sync::atomic::AtomicUsize;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use tokio::sync::Notify;
use tokio::task;
use uuid::Uuid;

#[tokio::test]
async fn listener_recreation_failure_drains_existing_client_work() {
    let approval = Arc::new(FakeApproval {
        release: Some(Arc::new(Notify::new())),
        ..Default::default()
    });
    let engine = Arc::new(Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::<FakeApproval>::clone(&approval),
        Arc::new(FakeStorage::default()),
    ));
    let endpoint = format!(r"\\.\pipe\teamy-broker-listener-failure-{}", Uuid::new_v4());
    let serving = Arc::clone(&engine);
    let server_endpoint = endpoint.clone();
    let server = tokio::spawn(async move {
        let recreated = AtomicUsize::new(0);
        serve_with_listener_factory(
            serving,
            CancellationToken::new(),
            &server_endpoint,
            false,
            |endpoint, first| {
                if !first && recreated.fetch_add(1, Ordering::SeqCst) == 2 {
                    eyre::bail!("synthetic listener recreation failure");
                }
                pipe_security::create(endpoint, first)
            },
        )
        .await
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(2));
    time::timeout(Duration::from_secs(2), async {
        while !client.status().await.unwrap().running {
            task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let acquiring = client.clone();
    let acquisition =
        tokio::spawn(async move { acquiring.acquire(support::request(Backend::Memory)).await });
    time::timeout(Duration::from_secs(2), async {
        while approval.calls.load(Ordering::SeqCst) == 0 {
            task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let _ = client.status().await.unwrap_err();
    let error = time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("synthetic listener recreation failure")
    );
    let _ = acquisition.await.unwrap().unwrap_err();
    let status = engine.status().await.unwrap();
    assert!(status.stopping);
    assert_eq!(status.active_requests, 0);
    assert_eq!(status.storage_operations, 0);
    assert_eq!(status.active_leases, 0);
}
