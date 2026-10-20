use super::DialogAttention;
use std::cell::Cell;

#[test]
fn creation_before_visibility_does_not_consume_attention() {
    let state = DialogAttention::default();
    assert!(
        state
            .request_using(
                false,
                || panic!("premature focus"),
                || panic!("premature observation"),
                || panic!("premature flash"),
                || panic!("premature sound")
            )
            .is_none()
    );
    let outcome = state
        .request_using(
            true,
            || true,
            || true,
            || panic!("unexpected flash"),
            || Ok(()),
        )
        .unwrap();
    assert!(outcome.visible_before && outcome.foreground_matches && outcome.sound_queued);
}

#[test]
fn refused_focus_flashes_once_and_later_visible_ticks_do_nothing() {
    let state = DialogAttention::default();
    let focus = Cell::new(0);
    let flash = Cell::new(0);
    let sound = Cell::new(0);
    let first = state
        .request_using(
            true,
            || {
                focus.set(focus.get() + 1);
                false
            },
            || false,
            || {
                flash.set(flash.get() + 1);
                Some(false)
            },
            || {
                sound.set(sound.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    assert!(!first.foreground_returned && !first.foreground_matches);
    assert_eq!(first.flash_previously_active, Some(false));
    for _ in 0..3 {
        assert!(
            state
                .request_using(
                    true,
                    || {
                        focus.set(focus.get() + 1);
                        false
                    },
                    || false,
                    || {
                        flash.set(flash.get() + 1);
                        Some(false)
                    },
                    || {
                        sound.set(sound.get() + 1);
                        Ok(())
                    }
                )
                .is_none()
        );
    }
    assert_eq!((focus.get(), flash.get(), sound.get()), (1, 1, 1));
}

#[test]
fn successful_focus_sounds_once_without_flashing() {
    let state = DialogAttention::default();
    let result = state
        .request_using(
            true,
            || true,
            || true,
            || panic!("unexpected flash"),
            || Ok(()),
        )
        .unwrap();
    assert!(result.foreground_returned && result.foreground_matches && result.sound_queued);
    assert_eq!(result.flash_previously_active, None);
    assert_eq!(result.sound_error_code, None);
}

#[test]
fn actual_foreground_mismatch_triggers_fallback_even_after_nonzero_return() {
    let result = DialogAttention::default()
        .request_using(true, || true, || false, || Some(false), || Ok(()))
        .unwrap();
    assert!(result.foreground_returned);
    assert!(!result.foreground_matches);
    assert_eq!(result.flash_previously_active, Some(false));
}

#[test]
fn already_matching_foreground_does_not_flash_after_zero_return() {
    let result = DialogAttention::default()
        .request_using(
            true,
            || false,
            || true,
            || panic!("unexpected flash"),
            || Ok(()),
        )
        .unwrap();
    assert!(!result.foreground_returned);
    assert!(result.foreground_matches);
    assert_eq!(result.flash_previously_active, None);
}

#[test]
fn sound_failure_captures_only_numeric_code_and_never_retries() {
    let state = DialogAttention::default();
    let result = state
        .request_using(true, || false, || false, || Some(true), || Err(5))
        .unwrap();
    assert!(!result.sound_queued);
    assert_eq!(result.sound_error_code, Some(5));
    assert_eq!(result.flash_previously_active, Some(true));
    assert!(
        state
            .request_using(
                true,
                || panic!("repeated focus"),
                || panic!("repeated observation"),
                || panic!("repeated flash"),
                || panic!("repeated sound")
            )
            .is_none()
    );
}
