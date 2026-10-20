use super::DialogPresentation;
use std::cell::Cell;
#[test]
fn hidden_startup_is_shown_once_before_attention_becomes_eligible() {
    let state = DialogPresentation::default();
    let visible = Cell::new(false);
    let shows = Cell::new(0);
    let outcome = state
        .present_using(
            || visible.get(),
            || {
                shows.set(shows.get() + 1);
                visible.set(true);
                Ok(())
            },
        )
        .unwrap();
    assert!(!outcome.visible_before && outcome.show_requested && outcome.visible_after);
    assert_eq!(outcome.show_succeeded, Some(true));
    assert_eq!(shows.get(), 1);
    assert!(
        state
            .present_using(
                || panic!("repeated observation"),
                || panic!("repeated show")
            )
            .is_none()
    );
}
#[test]
fn already_visible_dialog_is_not_shown_or_restored_again() {
    let state = DialogPresentation::default();
    let outcome = state
        .present_using(|| true, || panic!("visible dialog must not be changed"))
        .unwrap();
    assert!(outcome.visible_before && outcome.visible_after && !outcome.show_requested);
    assert_eq!(outcome.show_succeeded, None);
    assert!(
        state
            .present_using(
                || false,
                || panic!("later user hiding must not cause re-show")
            )
            .is_none()
    );
}
#[test]
fn failed_show_records_only_code_and_does_not_retry() {
    let state = DialogPresentation::default();
    let outcome = state.present_using(|| false, || Err(5)).unwrap();
    assert!(!outcome.visible_after);
    assert_eq!(outcome.show_succeeded, Some(false));
    assert_eq!(outcome.show_error_code, Some(5));
    assert!(
        state
            .present_using(|| false, || panic!("failed presentation must not loop"))
            .is_none()
    );
}
