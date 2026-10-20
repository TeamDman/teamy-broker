use super::PairingPromptState;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::time::Instant;
fn state(closed: bool, until: Instant) -> PairingPromptState {
    PairingPromptState {
        confirmation: None,
        code: zeroize::Zeroizing::new("SYNTHETIC ONLY".into()),
        closed: Arc::new(AtomicBool::new(closed)),
        until,
    }
}
#[test]
fn cancellation_precedes_presentation() {
    assert!(
        state(true, Instant::now() + Duration::from_secs(5))
            .on_timer(|| panic!("cancelled dialog must not show or request attention"))
    );
}
#[test]
fn hard_deadline_precedes_presentation() {
    assert!(
        state(false, Instant::now())
            .on_timer(|| panic!("expired dialog must not show or request attention"))
    );
}
#[test]
fn eligible_timer_invokes_presentation_and_debug_stays_redacted() {
    let state = state(false, Instant::now() + Duration::from_secs(5));
    let calls = std::cell::Cell::new(0);
    assert!(!state.on_timer(|| calls.set(calls.get() + 1)));
    assert_eq!(calls.get(), 1);
    assert_eq!(format!("{state:?}"), "PairingPromptState([redacted])");
}
