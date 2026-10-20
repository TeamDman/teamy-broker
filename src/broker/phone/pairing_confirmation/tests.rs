use super::*;
fn ready() -> Arc<Attempt> {
    let a = Attempt::new(Instant::now() + std::time::Duration::from_mins(1), 12345);
    a.publish(Zeroizing::new("ABCD 1234 DCBA 5678".into()))
        .unwrap();
    a
}
#[test]
fn exact_inspection_wrong_ids_and_redaction() {
    let a = ready();
    let _ = a.inspect("latest").expect_err("expected rejection");
    let _ = a
        .confirm(&uuid::Uuid::new_v4().to_string(), "ABCD 1234 DCBA 5678")
        .expect_err("expected rejection");
    let s = a.inspect(&a.id).unwrap();
    assert_eq!(s.remaining_wrong_attempts, 3);
    assert_eq!(s.comparison, "ABCD 1234 DCBA 5678");
    assert!(!format!("{a:?} {s:?}").contains("ABCD"));
}
#[test]
fn wrong_codes_bounded_replay_and_case() {
    let a = ready();
    for remaining in [2, 1] {
        assert!(!a.confirm(&a.id, "abcd 1234 DCBA 5678").unwrap());
        assert_eq!(
            a.inspect(&a.id).unwrap().remaining_wrong_attempts,
            remaining
        );
    }
    assert!(!a.confirm(&a.id, "wrong").unwrap());
    assert!(a.finished());
    assert!(!a.accepted());
    let _ = a.inspect(&a.id).expect_err("expected rejection");
    let _ = a
        .confirm(&a.id, "ABCD 1234 DCBA 5678")
        .expect_err("expected rejection");
}
#[test]
fn confirm_once_wipes_and_dismisses() {
    let a = ready();
    assert!(a.confirm(&a.id, "ABCD 1234 DCBA 5678").unwrap());
    assert!(a.accepted());
    assert!(a.finished());
    assert!(a.state.lock().unwrap().code.is_none());
    let _ = a
        .confirm(&a.id, "ABCD 1234 DCBA 5678")
        .expect_err("expected rejection");
    assert!(!a.native(false));
    assert!(a.accepted());
}
#[test]
fn expired_and_cancelled_are_not_confirmable() {
    let a = Attempt::new(
        Instant::now()
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap(),
        0,
    );
    let _ = a
        .publish(Zeroizing::new("ABCD 1234 DCBA 5678".into()))
        .expect_err("expected rejection");
    let _ = a.inspect(&a.id).expect_err("expected rejection");
    let a = ready();
    a.cancel();
    assert!(!a.native(true));
    let _ = a
        .confirm(&a.id, "ABCD 1234 DCBA 5678")
        .expect_err("expected rejection");
}
#[test]
fn concurrent_native_cli_first_valid_wins() {
    for _ in 0..100 {
        let a = ready();
        let gate = Arc::new(std::sync::Barrier::new(3));
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let n = std::thread::spawn(move || {
            g.wait();
            aa.native(false)
        });
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let c = std::thread::spawn(move || {
            g.wait();
            aa.confirm(&aa.id, "ABCD 1234 DCBA 5678").unwrap_or(false)
        });
        gate.wait();
        let native = n.join().unwrap();
        let cli = c.join().unwrap();
        assert_ne!(native, cli);
        assert_eq!(a.accepted(), cli);
    }
}
#[test]
fn revocation_cancels_even_claimed_comparison() {
    let a = ready();
    assert!(a.confirm(&a.id, "ABCD 1234 DCBA 5678").unwrap());
    a.cancel();
    assert!(!a.accepted());
    assert!(a.finished());
    let _ = a.inspect(&a.id).expect_err("expected rejection");
}
#[test]
fn unready_and_new_attempt_do_not_reuse_code() {
    let a = Attempt::new(Instant::now() + std::time::Duration::from_mins(1), 12345);
    let _ = a.inspect(&a.id).expect_err("expected rejection");
    let _ = a
        .confirm(&a.id, "anything")
        .expect_err("expected rejection");
    let old = ready();
    old.cancel();
    assert_ne!(old.id, a.id);
    let _ = a
        .confirm(&old.id, "ABCD 1234 DCBA 5678")
        .expect_err("expected rejection");
}

#[test]
fn published_expiry_wipes_and_blocks_native() {
    let mut a = ready();
    Arc::get_mut(&mut a).unwrap().until = Instant::now();
    let _ = a.inspect(&a.id).expect_err("expected rejection");
    assert!(a.state.lock().unwrap().code.is_none());
    assert!(!a.native(true));
    assert!(!a.accepted());
}
#[test]
fn cancel_before_comparison_ready() {
    let a = Attempt::new(Instant::now() + std::time::Duration::from_mins(1), 12345);
    a.cancel_pending(&a.id).unwrap();
    let _ = a
        .publish(Zeroizing::new("ABCD 1234 DCBA 5678".into()))
        .expect_err("expected rejection");
}
#[test]
fn native_and_cli_approve_exactly_once() {
    for _ in 0..50 {
        let a = ready();
        let gate = Arc::new(std::sync::Barrier::new(3));
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let n = std::thread::spawn(move || {
            g.wait();
            aa.native(true)
        });
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let c = std::thread::spawn(move || {
            g.wait();
            aa.confirm(&aa.id, "ABCD 1234 DCBA 5678").unwrap_or(false)
        });
        gate.wait();
        assert_ne!(n.join().unwrap(), c.join().unwrap());
        assert!(a.accepted());
    }
}
#[test]
fn confirmation_racing_revocation_never_survives_revoke() {
    for _ in 0..50 {
        let a = ready();
        let gate = Arc::new(std::sync::Barrier::new(3));
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let n = std::thread::spawn(move || {
            g.wait();
            aa.cancel();
        });
        let aa = Arc::clone(&a);
        let g = Arc::clone(&gate);
        let c = std::thread::spawn(move || {
            g.wait();
            let _ = aa.confirm(&aa.id, "ABCD 1234 DCBA 5678");
        });
        gate.wait();
        n.join().unwrap();
        c.join().unwrap();
        assert!(!a.accepted());
        let _ = a.inspect(&a.id).expect_err("expected rejection");
    }
}

#[test]
fn inspection_does_not_resolve_or_consume_budget() {
    let a = ready();
    for _ in 0..50 {
        let snapshot = a.inspect(&a.id).unwrap();
        assert_eq!(snapshot.remaining_wrong_attempts, 3);
        drop(snapshot);
    }
    assert!(!a.finished());
    assert!(!a.accepted());
    assert!(a.confirm(&a.id, "ABCD 1234 DCBA 5678").unwrap());
}
