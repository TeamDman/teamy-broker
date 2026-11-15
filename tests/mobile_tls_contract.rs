use rustls::crypto::ring;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::ServerName;
use std::env;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::process;
use std::str;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy::phone_profile::PhoneProfile;
use teamy_broker::broker::mobile_dummy::tailnet_peer::PORT;
use teamy_broker::broker::mobile_dummy::tailnet_peer::TailnetPeer;
use teamy_broker::broker::mobile_dummy::tls_identity::TlsIdentity;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker::broker::server;
use teamy_broker::paths::AppHome;
use teamy_broker::protocol::wire;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::pending::Pending;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use teamy_broker_handoff::wire as teamy_broker_handoff_wire;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task;
use tokio::time;
use tokio::time::Instant;
fn connector(cert: CertificateDer<'static>) -> tokio_rustls::TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).unwrap();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tokio_rustls::TlsConnector::from(Arc::new(config))
}
async fn tls_exchange(
    address: SocketAddr,
    connector: &tokio_rustls::TlsConnector,
    packet: &mobile_dummy::packet::Packet,
) -> eyre::Result<Envelope> {
    let tcp = TcpStream::connect(address).await?;
    let mut tls = connector
        .connect(ServerName::IpAddress(Ipv4Addr::LOCALHOST.into()), tcp)
        .await?;
    wire::write(&mut tls, packet).await?;
    tls.flush().await?;
    wire::read(&mut tls).await
}
#[tokio::test]
async fn tls_poll_submit_completes_existing_sdk_consumer_and_rejects_replay() {
    let session = Session::random().unwrap();
    let phone = Session::from_bootstrap(&session.bootstrap()).unwrap();
    let queue = Arc::new(Queue::new(session));
    let engine = Arc::new(Engine::new(
        Arc::new(mobile_dummy::adapter::MobileDummyAdapter),
        Arc::new(mobile_dummy::approval::MobileDummyApproval {
            queue: Arc::clone(&queue),
        }),
        Arc::new(mobile_dummy::storage::DummyMemoryStorage),
    ));
    let endpoint = format!(r"\\.\pipe\teamy-broker-tls-test-{}", uuid::Uuid::new_v4());
    let server_endpoint = endpoint.clone();
    let ipc = tokio::spawn(async move {
        server::serve_at(
            engine,
            teamy_cancellation::CancellationToken::new(),
            &server_endpoint,
            false,
        )
        .await
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(4));
    time::timeout(Duration::from_secs(3), async {
        while !client.status().await.unwrap().running {
            task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let identity = TlsIdentity::random().unwrap();
    let connector = connector(identity.certificate);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor,
        Duration::from_secs(10),
    ));
    let consumer = client.clone();
    let waiting = tokio::spawn(async move { consumer.acquire(mobile_dummy::request()).await });
    let challenge = teamy_broker_handoff_wire::random::<32>().unwrap();
    let poll = mobile_dummy::packet::Packet {
        operation: "poll".into(),
        envelope: phone
            .seal(teamy_broker_handoff::POLL_AAD, &challenge)
            .unwrap(),
    };
    let binding = time::timeout(Duration::from_secs(2), async {
        loop {
            let reply = tls_exchange(address, &connector, &poll).await.unwrap();
            let plain = phone.open(&challenge, &reply).unwrap();
            let pending: Vec<Pending> =
                facet_json::from_str(str::from_utf8(&plain).unwrap()).unwrap();
            if let Some(first) = pending.first() {
                break facet_json::from_str::<Binding>(&first.binding).unwrap();
            }
            task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(binding.consumer_pid, process::id());
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
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let session = Session::random().unwrap();
    let queue = Arc::new(Queue::new(session));
    let binding = Binding::dummy(42, wire::unix_ms().unwrap()).unwrap();
    let (id, _receiver) = queue.enqueue(binding).unwrap();
    let server = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor,
        Duration::from_secs(10),
    ));
    let tcp = TcpStream::connect(address).await.unwrap();
    let _ = wrong
        .connect(ServerName::IpAddress(Ipv4Addr::LOCALHOST.into()), tcp)
        .await
        .map(|_| ())
        .unwrap_err();
    let tcp = TcpStream::connect(address).await.unwrap();
    let _ = valid
        .connect(ServerName::try_from("wrong.invalid").unwrap(), tcp)
        .await
        .map(|_| ())
        .unwrap_err();
    let bad = Session::random().unwrap();
    let packet = mobile_dummy::packet::Packet {
        operation: "poll".into(),
        envelope: bad.seal(teamy_broker_handoff::POLL_AAD, &[0; 32]).unwrap(),
    };
    let _ = tls_exchange(address, &valid, &packet).await.unwrap_err();
    let mut plaintext = TcpStream::connect(address).await.unwrap();
    plaintext.write_all(b"invalid plaintext").await.unwrap();
    plaintext.shutdown().await.unwrap();
    let mut result = [0; 64];
    let count = plaintext.read(&mut result).await.unwrap_or(0);
    assert!(count == 0 || result[0] == 21);
    let tcp = TcpStream::connect(address).await.unwrap();
    let mut tls = valid
        .connect(ServerName::IpAddress(Ipv4Addr::LOCALHOST.into()), tcp)
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
    let profile = PhoneProfile {
        desktop: "100.64.0.1".parse().unwrap(),
        phone: "100.64.0.2".parse().unwrap(),
        desktop_node: "desktop-test".to_owned().try_into().unwrap(),
        phone_node: "phone-test".to_owned().try_into().unwrap(),
        user: 42.try_into().unwrap(),
        phone_name: "phone.invalid.".to_owned().try_into().unwrap(),
        java: env::temp_dir().join("synthetic-java").try_into().unwrap(),
    };
    assert!(TailnetPeer::permits_bind_for(
        (profile.desktop.get(), PORT).into(),
        &profile
    ));
    assert!(!TailnetPeer::permits_bind_for(
        (profile.desktop.get(), 443).into(),
        &profile
    ));
    assert!(TailnetPeer::permits_source_for(
        (profile.phone.get(), 12345).into(),
        &profile
    ));
    assert!(!TailnetPeer::permits_source_for(
        (profile.desktop.get(), 12345).into(),
        &profile
    ));
    let fixture = format!(
        r#"{{"Node":{{"StableID":"{}","Name":"{}","User":{},"KeyExpiry":"2099-01-01T00:00:00Z","Addresses":["{}/32"],"Hostinfo":{{"OS":"android"}}}}}}"#,
        profile.phone_node.as_str(),
        profile.phone_name.as_str(),
        profile.user.get(),
        profile.phone.get()
    );
    TailnetPeer::validate_whois(&fixture, &profile).unwrap();
    for (before, after) in [
        ("phone-test", "other-node"),
        ("android", "windows"),
        ("2099-01-01", "2000-01-01"),
    ] {
        assert!(TailnetPeer::validate_whois(&fixture.replace(before, after), &profile).is_err());
    }
}
#[test]
fn missing_phone_profile_is_rejected() {
    let path = env::temp_dir().join(format!("teamy-profile-missing-{}", uuid::Uuid::new_v4()));
    let _ = PhoneProfile::load(&AppHome(path)).unwrap_err();
}
#[tokio::test]
async fn finite_tls_listener_closes_stalled_handshakes_and_rejects_unbounded_lifetime() {
    let identity = TlsIdentity::random().unwrap();
    let queue = Arc::new(Queue::new(Session::random().unwrap()));
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(mobile_dummy::tls_server::serve(
        listener,
        Arc::clone(&queue),
        identity.acceptor.clone(),
        Duration::from_millis(50),
    ));
    let mut stalled = TcpStream::connect(address).await.unwrap();
    time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let mut byte = [0; 1];
    assert_eq!(stalled.read(&mut byte).await.unwrap_or(0), 0);
    for lifetime in [Duration::ZERO, Duration::from_secs(301)] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
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
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    assert!(
        mobile_dummy::tls_server::serve_until(
            listener,
            Arc::clone(&queue),
            identity.acceptor.clone(),
            Instant::now() - Duration::from_millis(1)
        )
        .await
        .is_err()
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    assert!(
        mobile_dummy::tls_server::serve_until(
            listener,
            queue,
            identity.acceptor,
            Instant::now() + Duration::from_secs(301)
        )
        .await
        .is_err()
    );
}
