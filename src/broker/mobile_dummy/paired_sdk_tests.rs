use super::java_pairing_worker::JavaPairingWorker;
use super::packet::Packet;
use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use crate::broker::engine::Engine;
use crate::sdk::Client;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::pending::Pending;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use zeroize::Zeroizing;
async fn exchange(
    socket: &mut tokio::net::TcpStream,
    phone: &mut JavaPairingWorker,
    operation: &str,
    envelope: Envelope,
) -> eyre::Result<Envelope> {
    let packet = Zeroizing::new(facet_json::to_string(&Packet {
        operation: operation.into(),
        envelope,
    })?);
    let encrypted = phone.call(6, packet.as_bytes()).await?;
    write_frame(socket, &encrypted).await?;
    let encrypted = read_frame(socket).await?;
    let plaintext = phone.call(7, &encrypted).await?;
    Ok(facet_json::from_str(std::str::from_utf8(&plaintext)?)?)
}
async fn pending_binding(
    socket: &mut tokio::net::TcpStream,
    phone: &mut JavaPairingWorker,
    session: &Session,
) -> eyre::Result<String> {
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let challenge = teamy_broker_handoff::wire::random::<32>()?;
            let poll = session.seal(teamy_broker_handoff::POLL_AAD, &challenge)?;
            let reply = exchange(socket, phone, "poll", poll).await?;
            let plain = session.open(&challenge, &reply)?;
            let pending: Vec<Pending> = facet_json::from_str(std::str::from_utf8(&plain)?)?;
            if let Some(pending) = pending.first() {
                break Ok::<_, eyre::Report>(pending.binding.clone());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await??;
    Ok(result)
}
async fn confirmed_workers(
    until: tokio::time::Instant,
) -> eyre::Result<(JavaPairingWorker, JavaPairingWorker)> {
    let java = crate::broker::mobile_dummy::java_pairing_worker::configured_java()?;
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    let binding = PairingBinding::dummy(crate::protocol::wire::unix_ms()?)?;
    let mut desktop = JavaPairingWorker::start(&java, &artifacts, &binding, until).await?;
    let mut phone = JavaPairingWorker::start_test_phone(&java, &artifacts, &binding, until).await?;
    let initial = phone.call(2, &[]).await?;
    desktop.call(1, &initial).await?;
    let reply = desktop.call(2, &[]).await?;
    phone.call(1, &reply).await?;
    let finish = phone.call(2, &[]).await?;
    desktop.call(1, &finish).await?;
    let a = phone.authentication_code().await?;
    let b = desktop.authentication_code().await?;
    eyre::ensure!(a.as_str() == b.as_str(), "test comparison mismatch");
    drop(a);
    drop(b);
    let pm = phone.call(4, &[]).await?;
    let dm = desktop.call(4, &[]).await?;
    phone.call(5, &dm).await?;
    desktop.call(5, &pm).await?;
    Ok((desktop, phone))
}
#[tokio::test]
async fn paired_loopback_socket_completes_isolated_sdk_consumer_and_purge() -> eyre::Result<()> {
    let until = tokio::time::Instant::now() + Duration::from_secs(20);
    let (mut desktop, mut phone) = confirmed_workers(until).await?;
    let session = Session::random()?;
    let key = session.transport_key();
    let encrypted = desktop.call(6, key.as_ref()).await?;
    let received = phone.call(7, &encrypted).await?;
    let client_session = Session::from_bootstrap(&teamy_broker_handoff::wire::hex(&received))?;
    drop(received);
    drop(key);
    let queue = Arc::new(Queue::new(session));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut socket = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
    let (server, _) = listener.accept().await?;
    drop(listener);
    let actor = PairedSocket {
        socket: server,
        worker: desktop,
        until,
    };
    let server_queue = Arc::clone(&queue);
    let tcp = tokio::spawn(actor.serve(server_queue));
    let engine = Arc::new(Engine::new(
        Arc::new(super::adapter::MobileDummyAdapter),
        Arc::new(super::approval::MobileDummyApproval { queue }),
        Arc::new(super::storage::DummyMemoryStorage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-paired-test-{}",
        uuid::Uuid::new_v4()
    );
    let server_endpoint = endpoint.clone();
    let ipc = tokio::spawn(async move {
        crate::broker::server::serve_at(
            engine,
            teamy_cancellation::CancellationToken::new(),
            &server_endpoint,
            false,
        )
        .await
    });
    let client = Client::new(None)?
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(5));
    tokio::time::timeout(Duration::from_secs(3), async {
        while !client.status().await?.running {
            tokio::task::yield_now().await;
        }
        Ok::<_, eyre::Report>(())
    })
    .await??;
    let consumer = client.clone();
    let waiting = tokio::spawn(async move { consumer.acquire(super::request()).await });
    let exact = pending_binding(&mut socket, &mut phone, &client_session).await?;
    let record: teamy_broker_handoff::binding::Binding = facet_json::from_str(&exact)?;
    eyre::ensure!(
        record.consumer_pid == std::process::id() && record.lease_ms == 900_000,
        "test binding mismatch"
    );
    let mut plain = Zeroizing::new(vec![1]);
    plain.extend_from_slice(teamy_broker_handoff::DUMMY_SECRET);
    let inner = client_session.seal(exact.as_bytes(), &plain)?;
    let mut outer = Zeroizing::new(exact.into_bytes());
    outer.push(0);
    outer.extend_from_slice(facet_json::to_string(&inner)?.as_bytes());
    let submit = client_session.seal(teamy_broker_handoff::SUBMIT_AAD, &outer)?;
    let ack = exchange(&mut socket, &mut phone, "submit", submit).await?;
    eyre::ensure!(
        client_session
            .open(inner.nonce.as_bytes(), &ack)?
            .as_slice()
            == b"accepted",
        "test ack mismatch"
    );
    let secret = waiting.await??;
    eyre::ensure!(
        secret.expose(|bytes| bytes == teamy_broker_handoff::DUMMY_SECRET)?,
        "test dummy grant mismatch"
    );
    eyre::ensure!(
        client.status().await?.active_leases == 1,
        "test lease missing"
    );
    client.purge(Some(secret.lease_id.clone())).await?;
    drop(secret);
    eyre::ensure!(
        client.status().await?.active_leases == 0,
        "test purge mismatch"
    );
    client.stop_force().await?;
    ipc.await??;
    drop(socket);
    eyre::ensure!(tcp.await?.is_err(), "disconnected session accepted");
    Ok(())
}

#[tokio::test]
async fn authenticated_preview_bridge_reconciles_and_rejects_stale_decisions() -> eyre::Result<()> {
    use crate::broker::shared_preview::SharedPreviewHub;
    use crate::protocol::ApprovalPreviewDisposition;
    use crate::protocol::SharedPreviewSnapshot;
    use std::sync::atomic::AtomicBool;
    let until = tokio::time::Instant::now() + Duration::from_secs(20);
    let (mut desktop, mut phone) = confirmed_workers(until).await?;
    let session = Session::random()?;
    let key = session.transport_key();
    let encrypted = desktop.call(6, key.as_ref()).await?;
    let received = phone.call(7, &encrypted).await?;
    let client_session = Session::from_bootstrap(&teamy_broker_handoff::wire::hex(&received))?;
    drop(received);
    drop(key);
    let engine = Arc::new(Engine::new(
        Arc::new(super::adapter::MobileDummyAdapter),
        Arc::new(super::approval::MobileDummyApproval {
            queue: Arc::new(Queue::new(Session::random()?)),
        }),
        Arc::new(super::storage::DummyMemoryStorage),
    ));
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-preview-bridge-test-{}",
        uuid::Uuid::new_v4()
    );
    let server_endpoint = endpoint.clone();
    let ipc = tokio::spawn(async move {
        crate::broker::server::serve_at(
            engine,
            teamy_cancellation::CancellationToken::new(),
            &server_endpoint,
            false,
        )
        .await
    });
    let client = Client::new(None)?
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(5));
    tokio::time::timeout(Duration::from_secs(2), async {
        while client.shared_preview_state().await.is_err() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let trace = Arc::new(super::preview_phase::PreviewTrace::memory());
    let (mut socket, tcp) =
        preview_socket(desktop, until, session, client.clone(), Arc::clone(&trace)).await?;
    let request = SharedPreviewHub::resident().begin(Arc::new(AtomicBool::new(false)))?;
    let exact = pending_binding(&mut socket, &mut phone, &client_session).await?;
    let snapshot: SharedPreviewSnapshot = facet_json::from_str(&exact)?;
    assert_eq!(snapshot.kind, "no-vault-preview-v1");
    assert!(!exact.contains("reference"));
    assert!(!exact.contains("lease"));
    let submit = |decision: u8| -> eyre::Result<(Envelope, String)> {
        let inner = client_session.seal(exact.as_bytes(), &[decision])?;
        let nonce = inner.nonce.clone();
        let mut outer = Zeroizing::new(exact.as_bytes().to_vec());
        outer.push(0);
        outer.extend_from_slice(facet_json::to_string(&inner)?.as_bytes());
        Ok((
            client_session.seal(teamy_broker_handoff::SUBMIT_AAD, &outer)?,
            nonce,
        ))
    };
    let (envelope, nonce) = submit(1)?;
    let reply = exchange(&mut socket, &mut phone, "submit", envelope).await?;
    assert_eq!(
        client_session.open(nonce.as_bytes(), &reply)?.as_slice(),
        b"accepted"
    );
    assert_eq!(
        request.outcome(),
        Some(ApprovalPreviewDisposition::ApprovedTest)
    );
    assert!(client.shared_preview_state().await?.pending.is_none());
    let (envelope, nonce) = submit(0)?;
    let reply = exchange(&mut socket, &mut phone, "submit", envelope).await?;
    assert_eq!(
        client_session.open(nonce.as_bytes(), &reply)?.as_slice(),
        b"stale"
    );
    let challenge = teamy_broker_handoff::wire::random::<32>()?;
    let poll = client_session.seal(teamy_broker_handoff::POLL_AAD, &challenge)?;
    let reply = exchange(&mut socket, &mut phone, "poll", poll).await?;
    let pending: Vec<Pending> = facet_json::from_str(std::str::from_utf8(
        &client_session.open(&challenge, &reply)?,
    )?)?;
    assert!(pending.is_empty());
    assert_eq!(client.status().await?.active_leases, 0);
    client.stop().await?;
    ipc.await??;
    let poll = client_session.seal(teamy_broker_handoff::POLL_AAD, &challenge)?;
    let _ = exchange(&mut socket, &mut phone, "poll", poll)
        .await
        .unwrap_err();
    assert!(tcp.await?.is_err());
    assert_preview_trace(&trace, &snapshot.request_id, &exact, &nonce);
    Ok(())
}

async fn preview_socket(
    desktop: JavaPairingWorker,
    until: tokio::time::Instant,
    session: Session,
    client: Client,
    trace: Arc<super::preview_phase::PreviewTrace>,
) -> eyre::Result<(
    tokio::net::TcpStream,
    tokio::task::JoinHandle<eyre::Result<()>>,
)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let socket = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
    let (server, _) = listener.accept().await?;
    drop(listener);
    let actor = PairedSocket {
        socket: server,
        worker: desktop,
        until,
    };
    Ok((
        socket,
        tokio::spawn(actor.serve_preview_traced(session, client, Some(trace))),
    ))
}

fn assert_preview_trace(
    trace: &super::preview_phase::PreviewTrace,
    request_id: &str,
    exact: &str,
    nonce: &str,
) {
    let records = trace.test_records().join("\n");
    for phase in [
        "PreviewPublished",
        "PreviewResolved",
        "PreviewStale",
        "PreviewWithdrawn",
    ] {
        assert!(records.contains(phase));
    }
    for forbidden in [
        request_id,
        exact,
        nonce,
        "authentication_code",
        "payload",
        "binding",
        "secret_ref",
    ] {
        assert!(!records.contains(forbidden));
    }
}
