#[path = "support/fake_adapter.rs"]
mod fake_adapter;
#[path = "support/fake_approval.rs"]
mod fake_approval;
#[path = "support/native_guard.rs"]
mod native_guard;
use teamy_broker::broker::lease::Lease;
use teamy_broker::broker::persistence;
use teamy_broker::platform::cache_reader;
use teamy_broker::platform::scheduler;
use teamy_broker::protocol::AcquireRequest;
use teamy_broker::protocol::Backend;
use zeroize::Zeroizing;
fn lease(backend: Backend) -> Lease {
    Lease::new(
        AcquireRequest {
            account: "synthetic-account".into(),
            secret_ref: "op://Synthetic/item/token".into(),
            purpose: "synthetic OS backend verification".into(),
            requesting_app: "synthetic test".into(),
            destination: "https://example.invalid".into(),
            backend,
        },
        Zeroizing::new(b"SYNTHETIC_SECRET_NEVER_LOG".to_vec()),
        teamy_broker::protocol::wire::unix_ms().unwrap(),
        std::time::Instant::now(),
    )
    .unwrap()
}
#[test]
#[ignore = "Requires explicit reviewed synthetic credential and scheduled-task execution"]
fn isolated_native_backends_register_before_publish_and_generation_cleanup_preserves_replacement() {
    for file_backend in [true, false] {
        let namespace = uuid::Uuid::new_v4().to_string();
        let directory = std::env::temp_dir().join(format!("teamy-broker-native-{namespace}"));
        let (backend, dir) = if file_backend {
            std::fs::create_dir(&directory).unwrap();
            (
                Backend::WellKnownFile(
                    directory
                        .join("synthetic.dpapi")
                        .to_string_lossy()
                        .into_owned(),
                ),
                Some(directory),
            )
        } else {
            (
                Backend::WindowsCredentialManager(format!(
                    "teamy-broker/synthetic-test/{namespace}"
                )),
                None,
            )
        };
        assert!(cache_reader::read(&backend).unwrap().is_none());
        let first = lease(backend.clone());
        let second = lease(backend.clone());
        let guard = native_guard::NativeGuard {
            ids: vec![first.id.clone(), second.id.clone()],
            backend: backend.clone(),
            directory: dir,
        };
        persistence::publish(&first).unwrap();
        assert!(scheduler::exists(&first.id).unwrap());
        let bytes = cache_reader::read(&backend).unwrap().unwrap();
        assert_eq!(
            uuid::Uuid::from_slice(&bytes[8..24]).unwrap().to_string(),
            first.id
        );
        if let Backend::WellKnownFile(path) = &backend {
            assert!(
                !std::fs::read(path)
                    .unwrap()
                    .windows(b"SYNTHETIC_SECRET".len())
                    .any(|part| part == b"SYNTHETIC_SECRET")
            );
        }
        persistence::publish(&second).unwrap();
        persistence::cleanup(&first.id, true).unwrap();
        assert!(!scheduler::exists(&first.id).unwrap());
        let bytes = cache_reader::read(&backend).unwrap().unwrap();
        assert_eq!(
            uuid::Uuid::from_slice(&bytes[8..24]).unwrap().to_string(),
            second.id
        );
        persistence::cleanup(&second.id, true).unwrap();
        assert!(cache_reader::read(&backend).unwrap().is_none());
        assert!(!scheduler::exists(&second.id).unwrap());
        drop(guard);
    }
}
#[test]
#[ignore = "Requires explicit reviewed synthetic scheduled-task execution"]
fn failed_native_publish_removes_its_task_and_preserves_foreign_destination() {
    let namespace = uuid::Uuid::new_v4().to_string();
    let directory = std::env::temp_dir().join(format!("teamy-broker-native-{namespace}"));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("synthetic.dpapi");
    std::fs::write(&path, b"synthetic foreign file").unwrap();
    let backend = Backend::WellKnownFile(path.to_string_lossy().into_owned());
    let item = lease(backend.clone());
    let guard = native_guard::NativeGuard {
        ids: vec![item.id.clone()],
        backend: backend.clone(),
        directory: Some(directory),
    };
    assert!(persistence::publish(&item).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"synthetic foreign file");
    assert!(!scheduler::exists(&item.id).unwrap());
    std::fs::remove_file(path).unwrap();
    drop(guard);
    let target =
        Backend::WindowsCredentialManager(format!("teamy-broker/synthetic-test/{namespace}"));
    let mut item = lease(target.clone());
    item.secret = Zeroizing::new(vec![42; 4096]);
    let guard = native_guard::NativeGuard {
        ids: vec![item.id.clone()],
        backend: target.clone(),
        directory: None,
    };
    assert!(persistence::publish(&item).is_err());
    assert!(cache_reader::read(&target).unwrap().is_none());
    assert!(!scheduler::exists(&item.id).unwrap());
    drop(guard);
}

#[tokio::test]
#[ignore = "Requires explicit reviewed synthetic credentials and scheduled tasks"]
async fn sdk_persistent_reads_fail_closed_after_purge_and_restart() {
    use std::sync::Arc;
    use teamy_broker::broker::engine::Engine;
    use teamy_broker::broker::native_storage::NativeStorage;
    use teamy_broker::broker::server::serve_at;
    use teamy_broker::sdk::Client;

    for file_backend in [true, false] {
        let namespace = uuid::Uuid::new_v4().to_string();
        let directory = std::env::temp_dir().join(format!("teamy-broker-native-{namespace}"));
        let (backend, dir) = if file_backend {
            std::fs::create_dir(&directory).unwrap();
            (
                Backend::WellKnownFile(
                    directory
                        .join("synthetic.dpapi")
                        .to_string_lossy()
                        .into_owned(),
                ),
                Some(directory),
            )
        } else {
            (
                Backend::WindowsCredentialManager(format!(
                    "teamy-broker/synthetic-test/{namespace}"
                )),
                None,
            )
        };
        let endpoint = format!(r"\\.\pipe\teamy-broker-test-{namespace}");
        let engine = Arc::new(Engine::new(
            Arc::new(fake_adapter::FakeAdapter::default()),
            Arc::new(fake_approval::FakeApproval::default()),
            Arc::new(NativeStorage),
        ));
        let cancel = teamy_cancellation::CancellationToken::new();
        let server = {
            let endpoint = endpoint.clone();
            tokio::spawn(async move { serve_at(engine, cancel, &endpoint, false).await })
        };
        let client = Client::new(None).unwrap().with_endpoint(endpoint.clone());
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
        let item = lease(backend.clone());
        let secret = client.acquire(item.request).await.unwrap();
        let guard = native_guard::NativeGuard {
            ids: vec![secret.lease_id.clone()],
            backend: backend.clone(),
            directory: dir,
        };
        let cached = client
            .load_cached(backend.clone(), secret.lease_id.clone())
            .await
            .unwrap();
        assert_eq!(
            cached.expose(<[u8]>::len).unwrap(),
            b"SYNTHETIC_SECRET_NEVER_LOG".len()
        );
        client.purge(Some(secret.lease_id.clone())).await.unwrap();
        let _ = client
            .load_cached(backend.clone(), secret.lease_id.clone())
            .await
            .unwrap_err();
        client.stop().await.unwrap();
        server.await.unwrap().unwrap();
        let _ = client
            .load_cached(backend.clone(), secret.lease_id.clone())
            .await
            .unwrap_err();
        assert_restarted_rejects(
            &client,
            endpoint.clone(),
            backend.clone(),
            secret.lease_id.clone(),
        )
        .await;
        assert!(cache_reader::read(&backend).unwrap().is_none());
        assert!(!scheduler::exists(&secret.lease_id).unwrap());
        drop(guard);
    }
}

async fn assert_restarted_rejects(
    client: &teamy_broker::sdk::Client,
    endpoint: String,
    backend: Backend,
    lease_id: String,
) {
    use std::sync::Arc;
    use teamy_broker::broker::engine::Engine;
    use teamy_broker::broker::native_storage::NativeStorage;
    use teamy_broker::broker::server::serve_at;
    let restarted = Arc::new(Engine::new(
        Arc::new(fake_adapter::FakeAdapter::default()),
        Arc::new(fake_approval::FakeApproval::default()),
        Arc::new(NativeStorage),
    ));
    let new_server = tokio::spawn(async move {
        serve_at(
            restarted,
            teamy_cancellation::CancellationToken::new(),
            &endpoint,
            false,
        )
        .await
    });
    while !client.status().await.unwrap().running {
        tokio::task::yield_now().await;
    }
    let _ = client.load_cached(backend, lease_id).await.unwrap_err();
    client.stop().await.unwrap();
    new_server.await.unwrap().unwrap();
}
