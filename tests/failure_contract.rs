mod support;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::server::serve_at;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::CAPABILITIES;
use teamy_broker::protocol::FailureKind;
use teamy_broker::protocol::Hello;
use teamy_broker::protocol::Response;
use teamy_broker::protocol::VERSION;
use teamy_broker::sdk::Client;
#[tokio::test]
async fn user_denial_timeout_and_upstream_failure_are_distinct_typed_redacted_ipc_errors() {
    for (kind, denied, timed_out, upstream) in [
        (FailureKind::UserDenied, true, false, false),
        (FailureKind::ApprovalTimedOut, false, true, false),
        (FailureKind::UpstreamReadFailed, false, false, true),
    ] {
        let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
        let adapter = Arc::new(FakeAdapter {
            fail: upstream,
            ..Default::default()
        });
        let engine = Arc::new(Engine::new(
            Arc::<FakeAdapter>::clone(&adapter),
            Arc::new(FakeApproval {
                denied,
                timed_out,
                ..Default::default()
            }),
            Arc::new(FakeStorage::default()),
        ));
        let cancel = teamy_cancellation::CancellationToken::new();
        let server = {
            let endpoint = endpoint.clone();
            tokio::spawn(async move { serve_at(engine, cancel, &endpoint, false).await })
        };
        let client = Client::new(None).unwrap().with_endpoint(endpoint);
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
        let error = client.acquire(request(Backend::Memory)).await.unwrap_err();
        assert_eq!(error.downcast_ref::<FailureKind>(), Some(&kind));
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(!text.contains("SYNTHETIC_UPSTREAM_DETAIL"));
            assert!(!text.contains("SYNTHETIC_SECRET"));
            assert!(!text.contains("op://"));
        }
        assert_eq!(adapter.calls.load(Ordering::SeqCst), usize::from(upstream));
        assert_eq!(client.status().await.unwrap().active_leases, 0);
        client.stop_force().await.unwrap();
        server.await.unwrap().unwrap();
    }
}
#[test]
fn typed_error_roundtrip_and_version_and_capability_compatibility_fail_closed() {
    for kind in [
        FailureKind::UserDenied,
        FailureKind::ApprovalTimedOut,
        FailureKind::UpstreamReadFailed,
    ] {
        let json = facet_json::to_string(&Response::failure(kind)).unwrap();
        let decoded: Response = facet_json::from_str(&json).unwrap();
        assert_eq!(decoded.failure, Some(kind));
        assert!(decoded.lease_id.is_none());
        assert!(decoded.expires_unix_ms.is_none());
    }
    let mut hello = Hello {
        version: VERSION,
        capabilities: CAPABILITIES.iter().map(|v| (*v).into()).collect(),
        instance_id: String::new(),
    };
    assert!(hello.compatible());
    hello.version = 1;
    assert!(!hello.compatible());
    hello.version = VERSION;
    hello
        .capabilities
        .retain(|cap| cap != "typed-acquisition-errors-v1");
    assert!(!hello.compatible());
}

#[tokio::test]
async fn v1_or_missing_typed_error_capability_rejects_before_autostart() {
    for old_version in [true, false] {
        let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
        let mut pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
        let peer = tokio::spawn(async move {
            pipe.connect().await.unwrap();
            let mut capabilities: Vec<String> = CAPABILITIES.iter().map(|v| (*v).into()).collect();
            if !old_version {
                capabilities.retain(|v| v != "typed-acquisition-errors-v1");
            }
            teamy_broker::protocol::wire::write(
                &mut pipe,
                &Hello {
                    version: if old_version { 1 } else { VERSION },
                    capabilities,
                    instance_id: String::new(),
                },
            )
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        });
        let client = Client::new(Some("must-never-spawn-incompatible-replacement.exe".into()))
            .unwrap()
            .with_endpoint(endpoint);
        let error = client.acquire(request(Backend::Memory)).await.unwrap_err();
        assert!(error.to_string().contains("incompatible"));
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn invalid_peer_failure_payload_cannot_leak_into_sdk_errors() {
    use tokio::io::AsyncWriteExt;
    let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
    let mut pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
    let peer = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        teamy_broker::protocol::wire::write(
            &mut pipe,
            &Hello {
                version: VERSION,
                capabilities: CAPABILITIES.iter().map(|v| (*v).into()).collect(),
                instance_id: String::new(),
            },
        )
        .await
        .unwrap();
        let _: Hello = teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
        let _: teamy_broker::protocol::Request =
            teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
        let invalid=br#"{"code":"failed","failure":"SYNTHETIC_UPSTREAM_SECRET_SHOULD_NOT_LEAK","lease_id":null,"expires_unix_ms":null,"remaining_millis":null,"status":null}"#;
        pipe.write_all(&u32::try_from(invalid.len()).unwrap().to_be_bytes())
            .await
            .unwrap();
        pipe.write_all(invalid).await.unwrap();
    });
    let error = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .acquire(request(Backend::Memory))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<FailureKind>(),
        Some(&FailureKind::RequestFailed)
    );
    assert!(!format!("{error:?}").contains("SYNTHETIC_UPSTREAM_SECRET"));
    peer.await.unwrap();
}
