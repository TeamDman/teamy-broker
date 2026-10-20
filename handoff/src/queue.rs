use crate::binding::Binding;
use crate::clock::Clock;
use crate::clock_reading::ClockReading;
use crate::deadline::Deadline;
use crate::entry::Entry;
use crate::envelope::Envelope;
use crate::pending::Pending;
use crate::received_decision::ReceivedDecision;
use crate::session::Session;
use crate::system_clock::SystemClock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::oneshot;
use zeroize::Zeroizing;
/// One bounded in-memory queue; no restart recovery or persistent pairing.
pub struct Queue {
    session: Session,
    entries: Mutex<HashMap<String, Entry>>,
    clock: Arc<dyn Clock>,
}
impl std::fmt::Debug for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Queue([redacted])")
    }
}
impl Queue {
    pub fn new(session: Session) -> Self {
        Self::with_clock(session, Arc::new(SystemClock))
    }
    pub fn with_clock(session: Session, clock: Arc<dyn Clock>) -> Self {
        Self {
            session,
            entries: Mutex::new(HashMap::new()),
            clock,
        }
    }
    pub fn enqueue(
        &self,
        binding: Binding,
    ) -> eyre::Result<(String, oneshot::Receiver<ReceivedDecision>)> {
        let reading = self.clock.reading()?;
        eyre::ensure!(
            binding.desktop == crate::DESKTOP
                && binding.phone == crate::PHONE
                && binding.consumer == crate::CONSUMER
                && binding.account == "synthetic-account"
                && binding.reference == crate::REFERENCE
                && binding.purpose == crate::PURPOSE
                && binding.destination == "https://example.invalid"
                && binding.scope == "memory-only"
                && binding.lease_ms == crate::LEASE_MS
                && binding.version == 1
                && binding.expires_unix_ms > reading.unix_ms
                && binding.expires_unix_ms - reading.unix_ms <= crate::REQUEST_MS,
            "dummy request required"
        );
        let deadline = Deadline {
            expires_unix_ms: binding.expires_unix_ms,
            tick: reading.tick + Duration::from_millis(binding.expires_unix_ms - reading.unix_ms),
        };
        let id = binding.request_id.clone();
        let text = binding.encode()?;
        let (sender, receiver) = oneshot::channel();
        let mut entries = self
            .entries
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("queue unavailable"))?;
        let reading = self.clock.reading()?;
        entries.retain(|_, entry| live(entry, reading));
        eyre::ensure!(deadline.valid_at(reading), "request expired");
        eyre::ensure!(
            entries.len() < 4 && !entries.contains_key(&id),
            "queue unavailable"
        );
        entries.insert(
            id.clone(),
            Entry {
                binding: text,
                deadline,
                sender,
            },
        );
        Ok((id, receiver))
    }
    pub fn cancel(&self, id: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(id);
        }
    }
    /// Resolution updates for the local UI. Expiry and receiver cancellation close the prompt.
    pub fn is_pending(&self, id: &str) -> eyre::Result<bool> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("queue unavailable"))?;
        let reading = self.clock.reading()?;
        entries.retain(|_, entry| live(entry, reading));
        Ok(entries.contains_key(id))
    }
    /// The same mutex arbitrates desktop and authenticated phone decisions atomically.
    /// Closing a desktop window must never call this method.
    pub fn resolve_desktop(
        &self,
        binding: &Binding,
        decision: crate::local_decision::LocalDecision,
    ) -> eyre::Result<()> {
        let exact = binding.encode()?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("queue unavailable"))?;
        let reading = self.clock.reading()?;
        entries.retain(|_, entry| live(entry, reading));
        let entry = entries
            .get(&binding.request_id)
            .ok_or_else(|| eyre::eyre!("request unavailable"))?;
        eyre::ensure!(entry.binding == exact, "binding rejected");
        let bytes = match decision {
            crate::local_decision::LocalDecision::SupplyDummy => {
                Some(Zeroizing::new(crate::DUMMY_SECRET.to_vec()))
            }
            crate::local_decision::LocalDecision::Deny
            | crate::local_decision::LocalDecision::Cancel => None,
        };
        eyre::ensure!(live(entry, self.clock.reading()?), "request expired");
        let entry = entries
            .remove(&binding.request_id)
            .ok_or_else(|| eyre::eyre!("request unavailable"))?;
        eyre::ensure!(live(&entry, self.clock.reading()?), "request expired");
        entry
            .sender
            .send(ReceivedDecision {
                bytes,
                deadline: entry.deadline,
                clock: Arc::clone(&self.clock),
            })
            .map_err(|_redacted_error| eyre::eyre!("consumer unavailable"))
    }
    /// Encrypted poll includes a fresh client challenge. Server response AAD binds that challenge.
    pub fn poll(&self, envelope: &Envelope) -> eyre::Result<Envelope> {
        let challenge = self.session.open(crate::POLL_AAD, envelope)?;
        eyre::ensure!(challenge.len() == 32, "invalid poll");
        let mut entries = self
            .entries
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("queue unavailable"))?;
        let reading = self.clock.reading()?;
        entries.retain(|_, entry| live(entry, reading));
        let pending: Vec<Pending> = entries
            .values()
            .map(|entry| Pending {
                binding: entry.binding.clone(),
            })
            .collect();
        let text = facet_json::to_string(&pending)
            .map_err(|_redacted_error| eyre::eyre!("invalid pending"))?;
        self.session.seal(&challenge, text.as_bytes())
    }
    /// Outer payload: binding UTF-8, NUL, inner Envelope JSON. Binding is inner AAD.
    /// An acknowledgment means queue delivery, not consumer grant success.
    pub fn submit(&self, envelope: &Envelope) -> eyre::Result<Envelope> {
        let outer = self.session.open(crate::SUBMIT_AAD, envelope)?;
        let split = outer
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| eyre::eyre!("invalid response"))?;
        let binding = std::str::from_utf8(&outer[..split])
            .map_err(|_redacted_error| eyre::eyre!("invalid response"))?;
        let supplied: Binding = facet_json::from_str(binding)
            .map_err(|_redacted_error| eyre::eyre!("invalid response"))?;
        let json = std::str::from_utf8(&outer[split + 1..])
            .map_err(|_redacted_error| eyre::eyre!("invalid response"))?;
        let inner: Envelope = facet_json::from_str(json)
            .map_err(|_redacted_error| eyre::eyre!("invalid response"))?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("queue unavailable"))?;
        let reading = self.clock.reading()?;
        entries.retain(|_, entry| live(entry, reading));
        let entry = entries
            .get(&supplied.request_id)
            .ok_or_else(|| eyre::eyre!("request unavailable"))?;
        eyre::ensure!(entry.binding == binding, "binding rejected");
        let plaintext = self.session.open(binding.as_bytes(), &inner)?;
        let bytes = match plaintext.as_slice() {
            [1, rest @ ..] if rest == crate::DUMMY_SECRET => Some(Zeroizing::new(rest.to_vec())),
            [0] | [2] => None,
            _ => eyre::bail!("only the fixed dummy value is accepted"),
        };
        let ack = self.session.seal(inner.nonce.as_bytes(), b"accepted")?;
        // Read a fresh clock after decrypt/ack work, immediately before consuming the entry.
        eyre::ensure!(live(entry, self.clock.reading()?), "request expired");
        let entry = entries
            .remove(&supplied.request_id)
            .ok_or_else(|| eyre::eyre!("request unavailable"))?;
        // Consumption and delivery are separate boundaries; never deliver a stale decision.
        eyre::ensure!(live(&entry, self.clock.reading()?), "request expired");
        let decision = ReceivedDecision {
            bytes,
            deadline: entry.deadline,
            clock: Arc::clone(&self.clock),
        };
        entry
            .sender
            .send(decision)
            .map_err(|_redacted_error| eyre::eyre!("consumer unavailable"))?;
        Ok(ack)
    }
}
fn live(entry: &Entry, reading: ClockReading) -> bool {
    entry.deadline.valid_at(reading) && !entry.sender.is_closed()
}
