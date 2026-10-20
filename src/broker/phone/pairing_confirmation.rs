//! One scoped, ephemeral confirmation; no secret-bearing Debug output or logging.
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    Pending,
    Matched,
    Rejected,
    Cancelled,
    Expired,
}
struct State {
    code: Option<Zeroizing<String>>,
    outcome: Outcome,
    wrong: u8,
}
pub(crate) struct Attempt {
    pub id: String,
    pub until: Instant,
    pub expires_unix_ms: u64,
    state: Mutex<State>,
    diagnostic: Mutex<std::sync::Weak<super::Controller>>,
    #[cfg(test)]
    events: Mutex<Option<Arc<super::faithful_integration::Events>>>,
}
impl std::fmt::Debug for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingAttempt([redacted])")
    }
}
impl Attempt {
    pub fn new(until: Instant, expires_unix_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            until,
            expires_unix_ms,
            diagnostic: Mutex::new(std::sync::Weak::new()),
            #[cfg(test)]
            events: Mutex::new(None),
            state: Mutex::new(State {
                code: None,
                outcome: Outcome::Pending,
                wrong: 0,
            }),
        })
    }
    pub fn track(&self, controller: std::sync::Weak<super::Controller>) {
        if let Ok(mut d) = self.diagnostic.lock() {
            *d = controller;
        }
    }
    pub fn stage(&self, stage: crate::protocol::phone_status::PhonePhase) {
        if let Ok(d) = self.diagnostic.lock()
            && let Some(c) = d.upgrade()
        {
            c.phase(stage);
        }
    }
    pub fn failure(&self, category: crate::protocol::phone_status::PhoneFailureCategory) {
        if let Ok(d) = self.diagnostic.lock()
            && let Some(c) = d.upgrade()
        {
            c.failure(category);
        }
    }
    pub fn transport_failure(&self, error: &std::io::Error) {
        if let Ok(d) = self.diagnostic.lock()
            && let Some(c) = d.upgrade()
        {
            c.transport_failure(error);
        }
    }
    #[cfg(test)]
    pub fn attach_events(&self, events: Arc<super::faithful_integration::Events>) {
        if let Ok(mut e) = self.events.lock() {
            *e = Some(events);
        }
    }
    #[cfg(test)]
    pub fn event(&self, label: &'static str) {
        if let Ok(e) = self.events.lock()
            && let Some(e) = &*e
        {
            e.record(label);
        }
    }
    fn check(&self, state: &mut State) -> eyre::Result<()> {
        if state.outcome == Outcome::Pending && Instant::now() >= self.until {
            state.outcome = Outcome::Expired;
            state.code = None;
        }
        eyre::ensure!(
            state.outcome == Outcome::Pending,
            "pairing attempt unavailable"
        );
        Ok(())
    }
    pub fn publish(&self, code: Zeroizing<String>) -> eyre::Result<()> {
        let mut s = self
            .state
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?;
        self.check(&mut s)?;
        eyre::ensure!(s.code.is_none(), "pairing comparison already published");
        s.code = Some(code);
        Ok(())
    }
    pub fn inspect(
        &self,
        id: &str,
    ) -> eyre::Result<crate::protocol::pairing_comparison::PairingInspection> {
        eyre::ensure!(id == self.id, "pairing attempt unavailable");
        let mut s = self
            .state
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?;
        self.check(&mut s)?;
        let code = s
            .code
            .as_ref()
            .ok_or_else(|| eyre::eyre!("comparison not ready"))?;
        Ok(crate::protocol::pairing_comparison::PairingInspection {
            attempt_id: self.id.clone(),
            expires_unix_ms: self.expires_unix_ms,
            comparison: code.to_string(),
            remaining_wrong_attempts: 3 - s.wrong,
        })
    }
    pub fn confirm(&self, id: &str, peer: &str) -> eyre::Result<bool> {
        eyre::ensure!(id == self.id, "pairing attempt unavailable");
        let mut s = self
            .state
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?;
        self.check(&mut s)?;
        let code = s
            .code
            .as_ref()
            .ok_or_else(|| eyre::eyre!("comparison not ready"))?;
        let same = peer.as_bytes() == code.as_bytes();
        if same {
            s.outcome = Outcome::Matched;
            s.code = None;
        } else {
            s.wrong += 1;
            if s.wrong >= 3 {
                s.outcome = Outcome::Rejected;
                s.code = None;
            }
        }
        Ok(same)
    }
    pub fn native(&self, accept: bool) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if self.check(&mut s).is_err() || s.code.is_none() {
            return false;
        }
        s.outcome = if accept {
            Outcome::Matched
        } else {
            Outcome::Rejected
        };
        s.code = None;
        true
    }
    pub fn finished(&self) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return true;
        };
        let _ = self.check(&mut s);
        s.outcome != Outcome::Pending
    }
    pub fn accepted(&self) -> bool {
        let Ok(s) = self.state.lock() else {
            return false;
        };
        s.outcome == Outcome::Matched && Instant::now() < self.until
    }
    pub fn cancel_pending(&self, id: &str) -> eyre::Result<()> {
        eyre::ensure!(id == self.id, "pairing attempt unavailable");
        let mut s = self
            .state
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?;
        self.check(&mut s)?;
        s.outcome = Outcome::Cancelled;
        s.code = None;
        Ok(())
    }
    pub fn cancel(&self) {
        #[cfg(test)]
        self.event("attempt_cancel");
        if let Ok(mut s) = self.state.lock() {
            s.outcome = Outcome::Cancelled;
            s.code = None;
        }
    }
}
#[cfg(test)]
mod tests;
