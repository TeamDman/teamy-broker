mod test_clock;
use std::sync::Arc;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::clock::Clock;
use teamy_broker_handoff::clock_reading::ClockReading;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use test_clock::TestClock;
fn response(phone: &Session, binding: &Binding) -> Envelope {
    let binding = binding.encode().unwrap();
    let mut plain = vec![1];
    plain.extend(teamy_broker_handoff::DUMMY_SECRET);
    let inner = phone.seal(binding.as_bytes(), &plain).unwrap();
    let mut outer = binding.into_bytes();
    outer.push(0);
    outer.extend(facet_json::to_string(&inner).unwrap().as_bytes());
    phone
        .seal(teamy_broker_handoff::SUBMIT_AAD, &outer)
        .unwrap()
}
#[test]
fn expiry_during_crypto_or_between_consume_and_delivery_rejects() {
    for monotonic in [false, true] {
        for stage in [1, 2] {
            let clock = Arc::new(TestClock::new(1000));
            let valid = clock.reading().unwrap();
            let desktop = Session::random().unwrap();
            let phone = Session::from_bootstrap(&desktop.bootstrap()).unwrap();
            let queue = Queue::with_clock(desktop, clock.clone());
            let binding = Binding::dummy(1234, 1000).unwrap();
            let (_, mut receiver) = queue.enqueue(binding.clone()).unwrap();
            let expired = ClockReading {
                unix_ms: if monotonic {
                    999
                } else {
                    binding.expires_unix_ms
                },
                tick: if monotonic {
                    valid.tick + std::time::Duration::from_millis(teamy_broker_handoff::REQUEST_MS)
                } else {
                    valid.tick
                },
            };
            let mut readings = vec![valid; stage];
            readings.push(expired);
            clock.script(&readings);
            assert!(queue.submit(&response(&phone, &binding)).is_err());
            assert!(receiver.try_recv().is_err());
        }
    }
}
#[test]
fn delivered_decision_cannot_approve_after_receiver_resumes_late() {
    for monotonic in [false, true] {
        let clock = Arc::new(TestClock::new(1000));
        let valid = clock.reading().unwrap();
        let desktop = Session::random().unwrap();
        let phone = Session::from_bootstrap(&desktop.bootstrap()).unwrap();
        let queue = Queue::with_clock(desktop, clock.clone());
        let binding = Binding::dummy(1234, 1000).unwrap();
        let (_, mut receiver) = queue.enqueue(binding.clone()).unwrap();
        queue.submit(&response(&phone, &binding)).unwrap();
        let provisional = receiver.try_recv().unwrap();
        clock.set(ClockReading {
            unix_ms: if monotonic {
                999
            } else {
                binding.expires_unix_ms
            },
            tick: if monotonic {
                valid.tick + std::time::Duration::from_millis(teamy_broker_handoff::REQUEST_MS)
            } else {
                valid.tick
            },
        });
        assert!(provisional.approved().is_err());
    }
}
