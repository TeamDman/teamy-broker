use crate::protocol::FailureKind;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
#[derive(Debug)]
pub(crate) struct ApprovalDialogState {
    pub work: Option<crate::broker::work_guard::WorkGuard>,
    pub dialog: std::sync::Mutex<Option<crate::broker::work_guard::WorkGuard>>,
    pub cancel: Arc<AtomicBool>,
    pub timed_out: AtomicBool,
    pub deadline: std::time::Instant,
}
impl ApprovalDialogState {
    pub fn created(&self) {
        if let Some(work) = &self.work {
            let mut dialog = self.dialog.lock().expect("dialog state lock");
            if dialog.is_none() {
                *dialog = Some(work.phase(crate::broker::work_kind::WorkKind::Dialog));
            }
        }
    }
    pub fn destroyed(&self) {
        self.dialog.lock().expect("dialog state lock").take();
    }

    pub fn should_close(&self, milliseconds: usize) -> bool {
        if self.cancel.load(Ordering::SeqCst) {
            return true;
        }
        if milliseconds >= 60_000 || std::time::Instant::now() >= self.deadline {
            self.timed_out.store(true, Ordering::SeqCst);
            return true;
        }
        false
    }
    /// Cancellation and the hard deadline always precede presentation work.
    pub fn on_timer(&self, milliseconds: usize, attention: impl FnOnce()) -> bool {
        if self.should_close(milliseconds) {
            return true;
        }
        attention();
        false
    }
    pub fn result(&self, approved: bool) -> eyre::Result<bool> {
        eyre::ensure!(
            !self.cancel.load(Ordering::SeqCst),
            FailureKind::RequestCancelled
        );
        eyre::ensure!(
            !self.timed_out.load(Ordering::SeqCst) && std::time::Instant::now() < self.deadline,
            FailureKind::ApprovalTimedOut
        );
        Ok(approved)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn timer_state(deadline: std::time::Instant) -> ApprovalDialogState {
        ApprovalDialogState {
            work: None,
            dialog: std::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            timed_out: AtomicBool::new(false),
            deadline,
        }
    }
    #[test]
    fn cancellation_precedes_timer_attention() {
        let state = timer_state(std::time::Instant::now() + std::time::Duration::from_mins(1));
        state.cancel.store(true, Ordering::SeqCst);
        assert!(state.on_timer(1, || panic!("cancelled dialog must not request attention")));
        assert!(!state.timed_out.load(Ordering::SeqCst));
    }
    #[test]
    fn elapsed_hard_deadline_precedes_timer_attention() {
        let state = timer_state(
            std::time::Instant::now()
                .checked_sub(std::time::Duration::from_millis(1))
                .unwrap(),
        );
        assert!(state.on_timer(1, || panic!("expired dialog must not request attention")));
        assert!(state.timed_out.load(Ordering::SeqCst));
    }
    #[test]
    fn native_timer_limit_precedes_attention_even_if_deadline_has_not_elapsed() {
        let state = timer_state(std::time::Instant::now() + std::time::Duration::from_mins(1));
        assert!(state.on_timer(60_000, || panic!(
            "expired native timer must not request attention"
        )));
        assert!(state.timed_out.load(Ordering::SeqCst));
    }
    #[test]
    fn eligible_timer_runs_attention_without_closing() {
        let state = timer_state(std::time::Instant::now() + std::time::Duration::from_mins(1));
        let called = std::cell::Cell::new(false);
        assert!(!state.on_timer(1, || called.set(true)));
        assert!(called.get());
    }
    #[test]
    fn timer_and_user_denial_are_distinct_and_cancellation_never_approves() {
        let state = ApprovalDialogState {
            work: None,
            dialog: std::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            timed_out: AtomicBool::new(false),
            deadline: std::time::Instant::now() + std::time::Duration::from_mins(1),
        };
        assert!(!state.should_close(59_999));
        assert!(!state.result(false).unwrap());
        assert!(state.should_close(60_000));
        assert_eq!(
            state
                .result(false)
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::ApprovalTimedOut)
        );
        state.cancel.store(true, Ordering::SeqCst);
        assert_eq!(
            state
                .result(true)
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::RequestCancelled)
        );
    }
    #[test]
    fn delayed_native_timer_cannot_approve_after_the_hard_deadline() {
        let state = ApprovalDialogState {
            work: None,
            dialog: std::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            timed_out: AtomicBool::new(false),
            deadline: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_millis(1))
                .unwrap(),
        };
        assert_eq!(
            state
                .result(true)
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::ApprovalTimedOut)
        );
    }
    #[test]
    fn actual_dialog_lifecycle_and_late_blocking_teardown_remain_visible() {
        use crate::broker::work_kind::WorkKind;
        let activity = Arc::new(crate::broker::work_state::WorkState::default());
        let request = activity.request().unwrap();
        let state = ApprovalDialogState {
            work: Some(request.phase(WorkKind::Approval)),
            dialog: std::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            timed_out: AtomicBool::new(false),
            deadline: std::time::Instant::now() + std::time::Duration::from_mins(1),
        };
        assert_eq!(activity.snapshot().counts[WorkKind::Dialog as usize], 0);
        state.created();
        state.created();
        assert_eq!(activity.snapshot().counts[WorkKind::Dialog as usize], 1);
        state.cancel.store(true, Ordering::SeqCst);
        drop(request);
        assert_eq!(
            activity
                .admit_stop(false, false, false)
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::ShutdownBlocked),
            "dropping an async request must not hide its still-open blocking dialog"
        );
        state.destroyed();
        state.destroyed();
        assert_eq!(activity.snapshot().counts[WorkKind::Dialog as usize], 0);
        assert_eq!(
            activity
                .admit_stop(false, false, false)
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::ShutdownBlocked),
            "blocking approval teardown is still work"
        );
        drop(state);
        activity.admit_stop(false, false, false).unwrap();
    }
}
