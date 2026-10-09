use std::time::Duration;
use std::time::Instant;

/// Warn once when a command exceeds its declared elapsed-time expectation.
///
/// Call [`Self::checkpoint`] at useful phase boundaries. Dropping the timer also
/// checks the elapsed time, including returns caused by errors or cancellation.
/// Checks only emit a tracing warning: they never stop work or change its result.
/// There is no background task, so a blocked operation is checked when it returns
/// or reaches its next checkpoint.
#[derive(Debug)]
pub struct CommandTimer {
    command: &'static str,
    phase: &'static str,
    expected: Duration,
    started: Instant,
    warned: bool,
}

impl CommandTimer {
    /// Start timing a command with an explicit, command-specific expectation.
    #[must_use]
    pub fn new(command: &'static str, expected: Duration) -> Self {
        Self {
            command,
            phase: "command",
            expected,
            started: Instant::now(),
            warned: false,
        }
    }

    /// Check elapsed time and remember this phase for the final check on drop.
    ///
    /// Use constant phase labels, such as `"dispatch"` or `"output"`, so warnings
    /// describe the work without recording user arguments or paths.
    pub fn checkpoint(&mut self, phase: &'static str) {
        self.phase = phase;
        self.check_elapsed(self.started.elapsed());
    }

    fn check_elapsed(&mut self, elapsed: Duration) {
        if self.warned || elapsed <= self.expected {
            return;
        }
        self.warned = true;
        tracing::warn!(
            command = self.command,
            phase = self.phase,
            elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            expected_ms = u64::try_from(self.expected.as_millis()).unwrap_or(u64::MAX),
            "Command exceeded its expected duration"
        );
    }
}

impl Drop for CommandTimer {
    fn drop(&mut self) {
        self.check_elapsed(self.started.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fmt;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tracing::Event;
    use tracing::Subscriber;
    use tracing::field::Field;
    use tracing::field::Visit;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::prelude::*;

    #[derive(Debug, Default)]
    struct Fields(BTreeMap<String, String>);

    impl Visit for Fields {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            self.0.insert(field.name().to_owned(), format!("{value:?}"));
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_owned(), value.to_owned());
        }
    }

    #[derive(Clone, Debug, Default)]
    struct Warnings(Arc<Mutex<Vec<Fields>>>);

    impl<S: Subscriber> Layer<S> for Warnings {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            assert_eq!(*event.metadata().level(), tracing::Level::WARN);
            let mut fields = Fields::default();
            event.record(&mut fields);
            self.0.lock().unwrap().push(fields);
        }
    }

    fn capture_warnings(action: impl FnOnce()) -> Vec<Fields> {
        let warnings = Warnings::default();
        let subscriber = tracing_subscriber::registry().with(warnings.clone());
        tracing::subscriber::with_default(subscriber, action);
        let mut captured = warnings.0.lock().unwrap();
        std::mem::take(&mut *captured)
    }

    #[test]
    fn exceeding_the_threshold_warns_once_with_structured_fields() {
        let warnings = capture_warnings(|| {
            let mut timer = CommandTimer::new("home show", Duration::from_secs(1));
            timer.check_elapsed(Duration::from_millis(999));
            timer.check_elapsed(Duration::from_secs(1));
            assert!(!timer.warned);
            timer.phase = "dispatch";
            timer.check_elapsed(Duration::from_millis(1_001));
            timer.check_elapsed(Duration::from_secs(2));
            timer.checkpoint("output");
        });

        assert_eq!(warnings.len(), 1);
        let fields = &warnings[0].0;
        assert_eq!(fields.get("command").unwrap(), "home show");
        assert_eq!(fields.get("phase").unwrap(), "dispatch");
        assert_eq!(fields.get("elapsed_ms").unwrap(), "1001");
        assert_eq!(fields.get("expected_ms").unwrap(), "1000");
    }

    #[test]
    fn checkpoint_warns_when_elapsed_time_has_exceeded_the_budget() {
        let warnings = capture_warnings(|| {
            let mut timer = CommandTimer::new("cache show", Duration::from_secs(1));
            timer.started = Instant::now().checked_sub(Duration::from_secs(2)).unwrap();
            timer.checkpoint("dispatch");
            timer.checkpoint("output");
        });
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0.get("phase").unwrap(), "dispatch");
    }

    #[test]
    fn drop_checks_the_last_phase_without_an_explicit_final_checkpoint() {
        let warnings = capture_warnings(|| {
            let mut timer = CommandTimer::new("home show", Duration::from_secs(1));
            timer.checkpoint("output");
            timer.started = Instant::now().checked_sub(Duration::from_secs(2)).unwrap();
        });
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0.get("phase").unwrap(), "output");
    }

    #[test]
    fn early_error_keeps_its_result_and_still_checks_on_drop() {
        fn failing_command() -> Result<(), &'static str> {
            let mut timer = CommandTimer::new("cache open", Duration::from_secs(1));
            timer.checkpoint("dispatch");
            timer.started = Instant::now().checked_sub(Duration::from_secs(2)).unwrap();
            Err("fixture failure")?;
            Ok(())
        }

        let warnings = capture_warnings(|| {
            assert_eq!(failing_command(), Err("fixture failure"));
        });
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0.get("phase").unwrap(), "dispatch");
    }

    #[test]
    fn fast_commands_remain_silent_at_checkpoints_and_on_drop() {
        let warnings = capture_warnings(|| {
            let mut timer = CommandTimer::new("home show", Duration::MAX);
            timer.checkpoint("dispatch");
            timer.checkpoint("output");
        });
        assert!(warnings.is_empty());
    }

    #[test]
    fn cancellation_keeps_its_reason_and_still_checks_on_drop() {
        fn cancelled_command(token: &teamy_cancellation::CancellationToken) -> eyre::Result<()> {
            let mut timer = CommandTimer::new("home show", Duration::from_secs(1));
            timer.checkpoint("dispatch");
            timer.started = Instant::now().checked_sub(Duration::from_secs(2)).unwrap();
            token.bail_if_cancelled()?;
            Ok(())
        }

        let token = teamy_cancellation::CancellationToken::new();
        token.request_cancel("fixture cancellation");
        let warnings = capture_warnings(|| {
            let error = cancelled_command(&token).expect_err("cancelled command must return error");
            assert_eq!(error.to_string(), "fixture cancellation");
        });
        assert!(token.is_cancelled());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0.get("phase").unwrap(), "dispatch");
    }
}
