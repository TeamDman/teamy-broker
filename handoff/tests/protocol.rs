mod test_clock;
use std::sync::Arc;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::clock::Clock;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use teamy_broker_handoff::wire;
fn setup() -> (Queue, Session, Arc<test_clock::TestClock>) {
    let desktop = Session::random().unwrap();
    let phone = Session::from_bootstrap(&desktop.bootstrap()).unwrap();
    let clock = Arc::new(test_clock::TestClock::new(1000));
    clock.script(&[]);
    (Queue::with_clock(desktop, clock.clone()), phone, clock)
}
fn response(phone: &Session, binding: &Binding, decision: &[u8]) -> Envelope {
    let binding = binding.encode().unwrap();
    let inner = phone.seal(binding.as_bytes(), decision).unwrap();
    let mut bytes = binding.into_bytes();
    bytes.push(0);
    bytes.extend(facet_json::to_string(&inner).unwrap().as_bytes());
    phone
        .seal(teamy_broker_handoff::SUBMIT_AAD, &bytes)
        .unwrap()
}
fn dummy_decision() -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend(teamy_broker_handoff::DUMMY_SECRET);
    bytes
}
#[test]
fn approve_once_and_authenticate_poll() {
    let (queue, phone, _clock) = setup();
    let binding = Binding::dummy(1234, 1000).unwrap();
    let (_, mut receiver) = queue.enqueue(binding.clone()).unwrap();
    let challenge = wire::random::<32>().unwrap();
    let poll = phone
        .seal(teamy_broker_handoff::POLL_AAD, &challenge)
        .unwrap();
    let envelope = queue.poll(&poll).unwrap();
    let pending = phone.open(&challenge, &envelope).unwrap();
    let pending: Vec<teamy_broker_handoff::pending::Pending> =
        facet_json::from_str(std::str::from_utf8(&pending).unwrap()).unwrap();
    assert_eq!(pending[0].binding, binding.encode().unwrap());
    assert!(phone.open(b"wrong challenge", &envelope).is_err());
    let response = response(&phone, &binding, &dummy_decision());
    queue.submit(&response).unwrap();
    assert!(receiver.try_recv().unwrap().approved().unwrap());
    assert!(queue.submit(&response).is_err());
}
#[test]
fn wrong_peer_and_all_binding_tampering_fail_closed() {
    let (queue, phone, _clock) = setup();
    let binding = Binding::dummy(1234, 1000).unwrap();
    let (_, _receiver) = queue.enqueue(binding.clone()).unwrap();
    let wrong = Session::random().unwrap();
    assert!(
        queue
            .submit(&response(&wrong, &binding, &dummy_decision()))
            .is_err()
    );
    for field in 0..12 {
        let mut changed = binding.clone();
        match field {
            0 => changed.request_id.push('0'),
            1 => changed.request_nonce.push('0'),
            2 => changed.desktop.push('x'),
            3 => changed.phone.push('x'),
            4 => changed.consumer.push('x'),
            5 => changed.reference.push('x'),
            6 => changed.purpose.push('x'),
            7 => changed.scope.push('x'),
            8 => changed.expires_unix_ms += 1,
            9 => changed.consumer_pid += 1,
            10 => changed.destination.push('x'),
            _ => changed.lease_ms += 1,
        }
        assert!(
            queue
                .submit(&response(&phone, &changed, &dummy_decision()))
                .is_err()
        );
    }
    queue
        .submit(&response(&phone, &binding, &dummy_decision()))
        .unwrap();
}
#[test]
fn deny_cancel_expiry_disconnect_and_restart() {
    for decision in [0, 2] {
        let (queue, phone, _clock) = setup();
        let binding = Binding::dummy(1, 1000).unwrap();
        let (_, mut receiver) = queue.enqueue(binding.clone()).unwrap();
        queue
            .submit(&response(&phone, &binding, &[decision]))
            .unwrap();
        assert!(!receiver.try_recv().unwrap().approved().unwrap());
    }
    let (queue, phone, clock) = setup();
    let binding = Binding::dummy(1, 1000).unwrap();
    let (id, receiver) = queue.enqueue(binding.clone()).unwrap();
    let packet = response(&phone, &binding, &dummy_decision());
    queue.cancel(&id);
    assert!(queue.submit(&packet).is_err());
    drop(receiver);
    let (_, receiver) = queue.enqueue(binding.clone()).unwrap();
    drop(receiver);
    assert!(queue.submit(&packet).is_err());
    let (_, _receiver) = queue.enqueue(binding.clone()).unwrap();
    let mut expired = clock.reading().unwrap();
    expired.unix_ms = binding.expires_unix_ms;
    clock.set(expired);
    assert!(queue.submit(&packet).is_err());
    let restarted = Queue::with_clock(
        Session::from_bootstrap(&phone.bootstrap()).unwrap(),
        clock.clone(),
    );
    assert!(restarted.submit(&packet).is_err());
    let (new_session, _, _) = setup();
    assert!(new_session.submit(&packet).is_err());
}
#[test]
fn only_dummy_and_bounded_expiry_are_accepted() {
    let (queue, phone, _clock) = setup();
    let mut binding = Binding::dummy(1, 1000).unwrap();
    binding.expires_unix_ms += 1;
    assert!(queue.enqueue(binding.clone()).is_err());
    binding.expires_unix_ms -= 1;
    let (_, mut receiver) = queue.enqueue(binding.clone()).unwrap();
    assert!(
        queue
            .submit(&response(&phone, &binding, b"\x01NOT-A-CREDENTIAL"))
            .is_err()
    );
    assert!(receiver.try_recv().is_err());
    queue
        .submit(&response(&phone, &binding, &dummy_decision()))
        .unwrap();
    assert!(receiver.try_recv().unwrap().approved().unwrap());
}
#[test]
fn monotonic_expiry_cannot_be_extended_by_wall_clock_rollback() {
    let (queue, phone, clock) = setup();
    let mut binding = Binding::dummy(1, 1000).unwrap();
    binding.expires_unix_ms = 1001;
    let (_, _receiver) = queue.enqueue(binding.clone()).unwrap();
    let mut expired = clock.reading().unwrap();
    expired.unix_ms = 999;
    expired.tick += std::time::Duration::from_millis(1);
    clock.set(expired);
    assert!(
        queue
            .submit(&response(&phone, &binding, &dummy_decision()))
            .is_err()
    );
}
#[test]
fn standard_aes256_gcm_vector_matches_java() {
    let session = Session::from_bootstrap(&"00".repeat(32)).unwrap();
    let vector = Envelope {
        nonce: "00".repeat(12),
        ciphertext: "cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919".into(),
    };
    assert_eq!(session.open(b"", &vector).unwrap().as_slice(), &[0u8; 16]);
    let wrong = Envelope {
        nonce: vector.nonce,
        ciphertext: format!("01{}", &vector.ciphertext[2..]),
    };
    assert!(session.open(b"", &wrong).is_err());
}
#[test]
fn debug_is_redacted_and_malformed_envelopes_are_safe() {
    let (queue, phone, _clock) = setup();
    assert_eq!(format!("{phone:?}"), "Session([redacted])");
    assert_eq!(format!("{queue:?}"), "Queue([redacted])");
    for nonce in ["", "zz", "0", "00"] {
        assert!(
            phone
                .open(
                    b"",
                    &Envelope {
                        nonce: nonce.into(),
                        ciphertext: "00".into()
                    }
                )
                .is_err()
        );
    }
}
