use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy::adapter::MobileDummyAdapter;
use teamy_broker::broker::mobile_dummy::approval::MobileDummyApproval;
use teamy_broker::broker::mobile_dummy::storage::DummyMemoryStorage;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
fn setup() -> (Arc<Engine>, Arc<Queue>, Session) {
    let session = Session::random().unwrap();
    let phone = Session::from_bootstrap(&session.bootstrap()).unwrap();
    let queue = Arc::new(Queue::new(session));
    let engine = Arc::new(Engine::new(
        Arc::new(MobileDummyAdapter),
        Arc::new(MobileDummyApproval {
            queue: Arc::clone(&queue),
        }),
        Arc::new(DummyMemoryStorage),
    ));
    (engine, queue, phone)
}
async fn pending(queue: &Queue, phone: &Session) -> Binding {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let challenge = teamy_broker_handoff::wire::random::<32>().unwrap();
            let poll = phone
                .seal(teamy_broker_handoff::POLL_AAD, &challenge)
                .unwrap();
            let envelope = queue.poll(&poll).unwrap();
            let plain = phone.open(&challenge, &envelope).unwrap();
            let entries: Vec<teamy_broker_handoff::pending::Pending> =
                facet_json::from_str(std::str::from_utf8(&plain).unwrap()).unwrap();
            if let Some(entry) = entries.first() {
                return facet_json::from_str(&entry.binding).unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
fn submit(
    queue: &Queue,
    phone: &Session,
    binding: &Binding,
    decision: u8,
) -> eyre::Result<Envelope> {
    let binding = binding.encode()?;
    let mut plain = vec![decision];
    if decision == 1 {
        plain.extend(teamy_broker_handoff::DUMMY_SECRET);
    }
    let envelope = phone.seal(binding.as_bytes(), &plain)?;
    let mut outer = binding.into_bytes();
    outer.push(0);
    outer.extend(facet_json::to_string(&envelope)?.as_bytes());
    queue.submit(&phone.seal(teamy_broker_handoff::SUBMIT_AAD, &outer)?)
}
#[tokio::test]
async fn mobile_approval_finishes_waiting_consumer_with_fixed_memory_lease() {
    let (engine, queue, phone) = setup();
    let worker = Arc::clone(&engine);
    let task = tokio::spawn(async move {
        worker
            .acquire(
                mobile_dummy::request(),
                42,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    });
    let binding = pending(&queue, &phone).await;
    assert_eq!(binding.consumer_pid, 42);
    let before = teamy_broker::protocol::wire::unix_ms().unwrap();
    submit(&queue, &phone, &binding, 1).unwrap();
    let grant = task.await.unwrap().unwrap();
    assert_eq!(grant.bytes.as_slice(), teamy_broker_handoff::DUMMY_SECRET);
    assert!(grant.expires >= before + 900_000);
    assert!(
        grant
            .deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_secs()
            <= 900
    );
    let _ = submit(&queue, &phone, &binding, 1).unwrap_err();
    engine.validate_lease(&grant.id).await.unwrap();
    let cached = engine
        .acquire(
            mobile_dummy::request(),
            42,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(cached.id, grant.id);
    assert_eq!(cached.expires, grant.expires);
    assert_eq!(cached.deadline, grant.deadline);
    drop(cached);
    engine.purge(Some(&grant.id)).await.unwrap();
    let _ = engine.validate_lease(&grant.id).await.unwrap_err();
    assert_eq!(teamy_broker::protocol::VERSION, 2);
}
#[tokio::test]
async fn deny_and_cancel_create_no_lease() {
    for decision in [0, 2] {
        let (engine, queue, phone) = setup();
        let worker = Arc::clone(&engine);
        let task = tokio::spawn(async move {
            worker
                .acquire(
                    mobile_dummy::request(),
                    42,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
        });
        let binding = pending(&queue, &phone).await;
        submit(&queue, &phone, &binding, decision).unwrap();
        let _ = task.await.unwrap().unwrap_err();
        assert_eq!(engine.status().await.unwrap().active_leases, 0);
    }
}
#[tokio::test]
async fn desktop_cancel_invalidates_pending_response() {
    let (engine, queue, phone) = setup();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker = Arc::clone(&engine);
    let token = Arc::clone(&cancel);
    let task =
        tokio::spawn(async move { worker.acquire(mobile_dummy::request(), 42, token).await });
    let binding = pending(&queue, &phone).await;
    cancel.store(true, Ordering::SeqCst);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    let _ = submit(&queue, &phone, &binding, 1).unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}
#[tokio::test]
async fn purge_epoch_prevents_an_approved_pending_request_from_granting() {
    let (engine, queue, phone) = setup();
    let worker = Arc::clone(&engine);
    let task = tokio::spawn(async move {
        worker
            .acquire(
                mobile_dummy::request(),
                42,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    });
    let binding = pending(&queue, &phone).await;
    engine.purge(None).await.unwrap();
    submit(&queue, &phone, &binding, 1).unwrap();
    let _ = task.await.unwrap().unwrap_err();
    assert_eq!(engine.status().await.unwrap().active_leases, 0);
}

#[tokio::test]
async fn mobile_handoff_works_through_existing_sdk_and_isolated_named_pipe() {
    let (engine, queue, phone) = setup();
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-mobile-test-{}",
        uuid::Uuid::new_v4()
    );
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
        .with_deadlines(
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(3),
        );
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !client.status().await.unwrap().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let consumer = client.clone();
    let waiting = tokio::spawn(async move { consumer.acquire(mobile_dummy::request()).await });
    let binding = pending(&queue, &phone).await;
    assert_eq!(binding.consumer_pid, std::process::id());
    submit(&queue, &phone, &binding, 1).unwrap();
    let secret = waiting.await.unwrap().unwrap();
    assert!(
        secret
            .expose(|bytes| bytes == teamy_broker_handoff::DUMMY_SECRET)
            .unwrap()
    );
    assert_eq!(client.status().await.unwrap().active_leases, 1);
    client.purge(Some(secret.lease_id.clone())).await.unwrap();
    drop(secret);
    client.stop_force().await.unwrap();
    ipc.await.unwrap().unwrap();
}

#[tokio::test]
async fn tcp_poll_and_submit_preserve_encrypted_protocol_framing() {
    let (engine, queue, phone) = setup();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let tcp_queue = Arc::clone(&queue);
    let tcp = tokio::spawn(mobile_dummy::server::serve(listener, tcp_queue));
    let waiting = tokio::spawn(async move {
        engine
            .acquire(
                mobile_dummy::request(),
                42,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    });
    let binding = pending(&queue, &phone).await;
    let challenge = teamy_broker_handoff::wire::random::<32>().unwrap();
    let packet = mobile_dummy::packet::Packet {
        operation: "poll".into(),
        envelope: phone
            .seal(teamy_broker_handoff::POLL_AAD, &challenge)
            .unwrap(),
    };
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    teamy_broker::protocol::wire::write(&mut stream, &packet)
        .await
        .unwrap();
    let reply: Envelope = teamy_broker::protocol::wire::read(&mut stream)
        .await
        .unwrap();
    phone.open(&challenge, &reply).unwrap();
    let exact = binding.encode().unwrap();
    let mut plain = vec![1];
    plain.extend(teamy_broker_handoff::DUMMY_SECRET);
    let inner = phone.seal(exact.as_bytes(), &plain).unwrap();
    let mut outer = exact.into_bytes();
    outer.push(0);
    outer.extend(facet_json::to_string(&inner).unwrap().as_bytes());
    let packet = mobile_dummy::packet::Packet {
        operation: "submit".into(),
        envelope: phone
            .seal(teamy_broker_handoff::SUBMIT_AAD, &outer)
            .unwrap(),
    };
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    teamy_broker::protocol::wire::write(&mut stream, &packet)
        .await
        .unwrap();
    let reply: Envelope = teamy_broker::protocol::wire::read(&mut stream)
        .await
        .unwrap();
    assert_eq!(
        phone
            .open(inner.nonce.as_bytes(), &reply)
            .unwrap()
            .as_slice(),
        b"accepted"
    );
    waiting.await.unwrap().unwrap();
    tcp.abort();
}
