mod test_clock;
#[test]
fn simultaneous_competing_decisions_deliver_exactly_one_result() {
    use std::sync::Barrier;
    for local in [
        LocalDecision::SupplyDummy,
        LocalDecision::Deny,
        LocalDecision::Cancel,
    ] {
        for _ in 0..12 {
            let session = Session::random().unwrap();
            let phone = Session::from_bootstrap(&session.bootstrap()).unwrap();
            let clock = Arc::new(test_clock::TestClock::new(1000));
            let queue = Arc::new(Queue::with_clock(session, clock));
            let binding = Binding::dummy(42, 1000).unwrap();
            let (id, mut receiver) = queue.enqueue(binding.clone()).unwrap();
            let mut bytes = vec![1];
            bytes.extend(teamy_broker_handoff::DUMMY_SECRET);
            let envelope = phone_response(&phone, &binding, &bytes);
            let barrier = Arc::new(Barrier::new(3));
            let (desktop_won, phone_won) = std::thread::scope(|scope| {
                let desktop_queue = Arc::clone(&queue);
                let desktop_barrier = Arc::clone(&barrier);
                let desktop = scope.spawn(move || {
                    desktop_barrier.wait();
                    desktop_queue.resolve_desktop(&binding, local).is_ok()
                });
                let phone_queue = Arc::clone(&queue);
                let phone_barrier = Arc::clone(&barrier);
                let mobile = scope.spawn(move || {
                    phone_barrier.wait();
                    phone_queue.submit(&envelope).is_ok()
                });
                barrier.wait();
                (desktop.join().unwrap(), mobile.join().unwrap())
            });
            assert_ne!(
                desktop_won, phone_won,
                "exactly one simultaneous contender must win"
            );
            assert_eq!(
                receiver.try_recv().unwrap().approved().unwrap(),
                phone_won || matches!(local, LocalDecision::SupplyDummy)
            );
            assert!(!queue.is_pending(&id).unwrap());
            assert!(receiver.try_recv().is_err());
        }
    }
}
use std::sync::Arc;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::local_decision::LocalDecision;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
fn phone_response(
    phone: &Session,
    binding: &Binding,
    decision: &[u8],
) -> teamy_broker_handoff::envelope::Envelope {
    let exact = binding.encode().unwrap();
    let inner = phone.seal(exact.as_bytes(), decision).unwrap();
    let mut outer = exact.into_bytes();
    outer.push(0);
    outer.extend(facet_json::to_string(&inner).unwrap().as_bytes());
    phone
        .seal(teamy_broker_handoff::SUBMIT_AAD, &outer)
        .unwrap()
}
#[test]
fn desktop_and_phone_decisions_share_one_atomic_winner() {
    for local in [
        LocalDecision::SupplyDummy,
        LocalDecision::Deny,
        LocalDecision::Cancel,
    ] {
        for phone_first in [false, true] {
            let session = Session::random().unwrap();
            let phone = Session::from_bootstrap(&session.bootstrap()).unwrap();
            let clock = Arc::new(test_clock::TestClock::new(1000));
            let queue = Queue::with_clock(session, clock);
            let binding = Binding::dummy(42, 1000).unwrap();
            let (id, mut receiver) = queue.enqueue(binding.clone()).unwrap();
            let mut bytes = vec![1];
            bytes.extend(teamy_broker_handoff::DUMMY_SECRET);
            let envelope = phone_response(&phone, &binding, &bytes);
            if phone_first {
                queue.submit(&envelope).unwrap();
                assert!(queue.resolve_desktop(&binding, local).is_err());
                assert!(receiver.try_recv().unwrap().approved().unwrap());
            } else {
                queue.resolve_desktop(&binding, local).unwrap();
                assert!(queue.submit(&envelope).is_err());
                assert_eq!(
                    receiver.try_recv().unwrap().approved().unwrap(),
                    matches!(local, LocalDecision::SupplyDummy)
                );
            }
            assert!(!queue.is_pending(&id).unwrap());
            assert!(queue.submit(&envelope).is_err());
        }
    }
}
#[test]
fn local_resolution_rejects_tampering_expiry_and_consumer_cancellation() {
    let clock = Arc::new(test_clock::TestClock::new(1000));
    let queue = Queue::with_clock(Session::random().unwrap(), clock.clone());
    let binding = Binding::dummy(42, 1000).unwrap();
    let (id, _receiver) = queue.enqueue(binding.clone()).unwrap();
    let mut changed = binding.clone();
    changed.consumer_pid += 1;
    assert!(
        queue
            .resolve_desktop(&changed, LocalDecision::SupplyDummy)
            .is_err()
    );
    assert!(queue.is_pending(&id).unwrap());
    // Dismissing a UI has no queue mutation; phone remains pending. Explicit cancel invalidates it.
    queue.cancel(&id);
    assert!(!queue.is_pending(&id).unwrap());
    assert!(
        queue
            .resolve_desktop(&binding, LocalDecision::SupplyDummy)
            .is_err()
    );
    let binding = Binding::dummy(42, 1000).unwrap();
    let (id, _receiver) = queue.enqueue(binding.clone()).unwrap();
    clock.script(&[teamy_broker_handoff::clock_reading::ClockReading {
        unix_ms: 61000,
        tick: std::time::Instant::now(),
    }]);
    assert!(
        queue
            .resolve_desktop(&binding, LocalDecision::SupplyDummy)
            .is_err()
    );
    assert!(!queue.is_pending(&id).unwrap());
}
