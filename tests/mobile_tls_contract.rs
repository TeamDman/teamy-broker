use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::mobile_dummy::tailnet_peer::TailnetPeer;
use teamy_broker::broker::mobile_dummy::tls_identity::TlsIdentity;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
fn connector(cert: rustls::pki_types::CertificateDer<'static>) -> tokio_rustls::TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).unwrap();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    tokio_rustls::TlsConnector::from(Arc::new(config))
}
async fn tls_exchange(
    address: std::net::SocketAddr,
    connector: &tokio_rustls::TlsConnector,
    packet: &mobile_dummy::packet::Packet,
) -> eyre::Result<Envelope> {
    let tcp = tokio::net::TcpStream::connect(address).await?;
    let mut tls = connector
        .connect(
            rustls::pki_types::ServerName::IpAddress(std::net::Ipv4Addr::LOCALHOST.into()),
            tcp,
        )
        .await?;
    teamy_broker::protocol::wire::write(&mut tls, packet).await?;
    tls.flush().await?;
    teamy_broker::protocol::wire::read(&mut tls).await
}
#[tokio::test]
async fn tls_poll_submit_completes_existing_sdk_consumer_and_rejects_replay() {
    let session = Session::random().unwrap();
    let phone = Session::from_bootstrap(&session.bootstrap()).unwrap();
    let queue = Arc::new(Queue::new(session));
    let engine = Arc::new(teamy_broker::broker::engine::Engine::new(
        Arc::new(mobile_dummy::adapter::MobileDummyAdapter),
        Arc::new(mobile_dummy::approval::MobileDummyApproval {
            queue: Arc::clone(&queue),
        }),
        Arc::new(mobile_dummy::storage::DummyMemoryStorage),
    ));
    let endpoint = format!(r"\\.\pipe\teamy-broker-tls-test-{}", uuid::Uuid::new_v4());
    let server_endpoint = endpoint.clone();
    let ipc = tokio::spawn(async move {
        teamy_broker::broker::server::serve_at(
            engine,
            teamy_cancellation::CancellationToken::new(),
            &server_endpoint,
            false,
        )
        .await
    });
    let client = teamy_broker::sdk::Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(4));
    tokio::time::timeout(Duration::from_secs(3), async {
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let identity = TlsIdentity::random().unwrap();
    let connector = connector(identity.certificate);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor,
        Duration::from_secs(10),
    ));
    let consumer = client.clone();
    let waiting = tokio::spawn(async move { consumer.acquire(mobile_dummy::request()).await });
    let challenge = teamy_broker_handoff::wire::random::<32>().unwrap();
    let poll = mobile_dummy::packet::Packet {
        operation: "poll".into(),
        envelope: phone
            .seal(teamy_broker_handoff::POLL_AAD, &challenge)
            .unwrap(),
    };
    let binding = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let reply = tls_exchange(address, &connector, &poll).await.unwrap();
            let plain = phone.open(&challenge, &reply).unwrap();
            let pending: Vec<teamy_broker_handoff::pending::Pending> =
                facet_json::from_str(std::str::from_utf8(&plain).unwrap()).unwrap();
            if let Some(first) = pending.first() {
                break facet_json::from_str::<Binding>(&first.binding).unwrap();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(binding.consumer_pid, std::process::id());
    let exact = binding.encode().unwrap();
    let mut plain = vec![1];
    plain.extend(teamy_broker_handoff::DUMMY_SECRET);
    let inner = phone.seal(exact.as_bytes(), &plain).unwrap();
    let mut outer = exact.into_bytes();
    outer.push(0);
    outer.extend(facet_json::to_string(&inner).unwrap().as_bytes());
    let submit = mobile_dummy::packet::Packet {
        operation: "submit".into(),
        envelope: phone
            .seal(teamy_broker_handoff::SUBMIT_AAD, &outer)
            .unwrap(),
    };
    let ack = tls_exchange(address, &connector, &submit).await.unwrap();
    assert_eq!(
        phone.open(inner.nonce.as_bytes(), &ack).unwrap().as_slice(),
        b"accepted"
    );
    let secret = waiting.await.unwrap().unwrap();
    assert!(
        secret
            .expose(|bytes| bytes == teamy_broker_handoff::DUMMY_SECRET)
            .unwrap()
    );
    let _ = tls_exchange(address, &connector, &submit)
        .await
        .unwrap_err();
    client.purge(Some(secret.lease_id.clone())).await.unwrap();
    drop(secret);
    client.stop_force().await.unwrap();
    ipc.await.unwrap().unwrap();
    server.abort();
}
#[tokio::test]
async fn wrong_certificate_hostname_session_plaintext_and_frames_fail_closed() {
    let identity = TlsIdentity::random().unwrap();
    let valid = connector(identity.certificate.clone());
    let wrong = connector(TlsIdentity::random().unwrap().certificate);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let session = Session::random().unwrap();
    let queue = Arc::new(Queue::new(session));
    let binding = Binding::dummy(42, teamy_broker::protocol::wire::unix_ms().unwrap()).unwrap();
    let (id, _receiver) = queue.enqueue(binding).unwrap();
    let server = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor,
        Duration::from_secs(10),
    ));
    let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let _ = wrong
        .connect(
            rustls::pki_types::ServerName::IpAddress(std::net::Ipv4Addr::LOCALHOST.into()),
            tcp,
        )
        .await
        .map(|_| ())
        .unwrap_err();
    let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let _ = valid
        .connect(
            rustls::pki_types::ServerName::try_from("wrong.invalid").unwrap(),
            tcp,
        )
        .await
        .map(|_| ())
        .unwrap_err();
    let bad = Session::random().unwrap();
    let packet = mobile_dummy::packet::Packet {
        operation: "poll".into(),
        envelope: bad.seal(teamy_broker_handoff::POLL_AAD, &[0; 32]).unwrap(),
    };
    let _ = tls_exchange(address, &valid, &packet).await.unwrap_err();
    let mut plaintext = tokio::net::TcpStream::connect(address).await.unwrap();
    plaintext.write_all(b"invalid plaintext").await.unwrap();
    plaintext.shutdown().await.unwrap();
    let mut result = [0; 64];
    let count = plaintext.read(&mut result).await.unwrap_or(0);
    assert!(count == 0 || result[0] == 21);
    let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut tls = valid
        .connect(
            rustls::pki_types::ServerName::IpAddress(std::net::Ipv4Addr::LOCALHOST.into()),
            tcp,
        )
        .await
        .unwrap();
    tls.write_u32(16385).await.unwrap();
    tls.flush().await.unwrap();
    let _ = tls.read_u8().await.unwrap_err();
    assert!(queue.is_pending(&id).unwrap());
    server.abort();
}
#[test]
fn peer_policy_rejects_other_nodes_and_binds_outside_the_approved_scope() {
    use teamy_broker_handoff::phone_config::DESKTOP;
    use teamy_broker_handoff::phone_config::PHONE;
    use teamy_broker_handoff::phone_config::PHONE_NAME;
    use teamy_broker_handoff::phone_config::PHONE_NODE;
    use teamy_broker_handoff::phone_config::PORT;
    use teamy_broker_handoff::phone_config::USER;
    assert!(TailnetPeer::permits_bind((DESKTOP, PORT).into()));
    for address in [
        "0.0.0.0:43188".parse().unwrap(),
        (DESKTOP, 443).into(),
        "127.0.0.1:43188".parse().unwrap(),
        "[::]:43188".parse().unwrap(),
    ] {
        assert!(!TailnetPeer::permits_bind(address));
    }
    assert!(TailnetPeer::permits_source((PHONE, 12345).into()));
    for source in ["127.0.0.1:12345", "192.0.2.200:12345", "[::1]:12345"] {
        assert!(!TailnetPeer::permits_source(source.parse().unwrap()));
    }
    let fixture = format!(
        r#"{{"Node":{{"StableID":"{PHONE_NODE}","Name":"{PHONE_NAME}","User":{USER},"KeyExpiry":"2099-01-01T00:00:00Z","Addresses":["{PHONE}/32"],"Hostinfo":{{"OS":"android"}}}},"UnusedField":true}}"#
    );
    TailnetPeer::validate_whois(&fixture).unwrap();
    for (before, after) in [
        (
            format!(r#""StableID":"{PHONE_NODE}""#),
            r#""StableID":"other-node""#.into(),
        ),
        (
            format!(r#""User":{USER}"#),
            format!(r#""User":{}"#, USER.saturating_add(1)),
        ),
        (r#""OS":"android""#.into(), r#""OS":"windows""#.into()),
        (format!("{PHONE}/32"), "127.0.0.1/32".into()),
        ("2099-01-01".into(), "2000-01-01".into()),
    ] {
        assert!(TailnetPeer::validate_whois(&fixture.replace(&before, &after)).is_err());
    }
}
#[tokio::test]
async fn unconfigured_phone_scope_rejects_activation() {
    if !teamy_broker_handoff::phone_config::CONFIGURED {
        assert!(TailnetPeer::verify_desktop().await.is_err());
    }
}
#[tokio::test]
async fn finite_tls_listener_closes_stalled_handshakes_and_rejects_unbounded_lifetime() {
    let identity = TlsIdentity::random().unwrap();
    let queue = Arc::new(Queue::new(Session::random().unwrap()));
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor.clone(),
        Duration::from_millis(50),
    ));
    let mut stalled = tokio::net::TcpStream::connect(address).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let mut byte = [0; 1];
    assert_eq!(stalled.read(&mut byte).await.unwrap_or(0), 0);
    for lifetime in [Duration::ZERO, Duration::from_secs(301)] {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        assert!(
            mobile_dummy::tls_server::serve(
                listener,
                Arc::clone(&queue),
                identity.acceptor.clone(),
                lifetime
            )
            .await
            .is_err()
        );
    }
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    assert!(
        mobile_dummy::tls_server::serve_until(
            listener,
            Arc::clone(&queue),
            identity.acceptor.clone(),
            tokio::time::Instant::now() - Duration::from_millis(1)
        )
        .await
        .is_err()
    );
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    assert!(
        mobile_dummy::tls_server::serve_until(
            listener,
            queue,
            identity.acceptor,
            tokio::time::Instant::now() + Duration::from_secs(301)
        )
        .await
        .is_err()
    );
}
