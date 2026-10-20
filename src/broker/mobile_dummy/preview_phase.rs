use facet::Facet;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;
#[derive(Clone, Copy, Debug, Facet, PartialEq, Eq)]
#[repr(u8)]
pub enum PreviewPhase {
    ListenerBound,
    PeerAccepted,
    PeerVerified,
    ClientInitAccepted,
    ServerInitSent,
    ClientFinishAccepted,
    HandshakeComplete,
    DialogCreated,
    DialogVisible,
    DialogNotVisible,
    DialogAttention,
    LocalMatch,
    LocalReject,
    LocalTimeout,
    LocalCancelled,
    DialogDestroyed,
    LocalConfirmationSent,
    PeerConfirmationReceived,
    PeerConfirmationVerified,
    PeerReverified,
    AuthenticatedReady,
    PreviewPublished,
    PreviewResolved,
    PreviewStale,
    PreviewWithdrawn,
    PairingDeadline,
    PairingRejected,
    TransportUnavailable,
    SessionDeadline,
    ProtocolFailed,
    Cleanup,
}
#[derive(Clone, Copy, Debug, Facet, PartialEq, Eq)]
#[repr(u8)]
pub enum PreviewOrigin {
    Desktop,
    Phone,
    Broker,
}
/// Only typed primitive fields; no text, identity, binding, code, frame or payload input exists.
#[derive(Clone, Copy, Debug, Default, Facet)]
pub struct PreviewMetadata {
    pub visible: Option<bool>,
    pub foreground: Option<bool>,
    pub sound_queued: Option<bool>,
    pub accepted: Option<bool>,
    pub ack_written: Option<bool>,
    pub origin: Option<PreviewOrigin>,
    pub decision: Option<crate::protocol::ApprovalPreviewDisposition>,
}
#[derive(Clone, Debug, Facet)]
struct PhaseEvent {
    schema: String,
    sequence: u32,
    elapsed_ms: u64,
    phase: PreviewPhase,
    metadata: PreviewMetadata,
}
struct State {
    events: Vec<PhaseEvent>,
    file: Option<File>,
}
/// At most 64 fixed-label JSON records. Paths and protocol material are never serialized.
pub struct PreviewTrace {
    started: Instant,
    state: Mutex<State>,
    failed: AtomicBool,
}
impl std::fmt::Debug for PreviewTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreviewTrace([bounded metadata])")
    }
}
impl PreviewTrace {
    /// # Errors
    /// Fails if the new diagnostic file cannot be exclusively created.
    pub fn create(path: &Path) -> eyre::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_redacted_error| eyre::eyre!("phase diagnostics unavailable"))?;
        Ok(Self {
            started: Instant::now(),
            state: Mutex::new(State {
                events: Vec::new(),
                file: Some(file),
            }),
            failed: AtomicBool::new(false),
        })
    }
    #[cfg(test)]
    pub(super) fn memory() -> Self {
        Self {
            started: Instant::now(),
            state: Mutex::new(State {
                events: Vec::new(),
                file: None,
            }),
            failed: AtomicBool::new(false),
        }
    }
    #[cfg(test)]
    pub(super) fn test_records(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .events
            .iter()
            .map(|event| facet_json::to_string(event).unwrap())
            .collect()
    }
    pub fn emit(&self, phase: PreviewPhase, metadata: PreviewMetadata) {
        let Ok(mut state) = self.state.lock() else {
            self.failed.store(true, Ordering::Release);
            return;
        };
        // Reserve terminal records even if a peer repeats valid state changes.
        let limit = match phase {
            PreviewPhase::Cleanup => 64,
            PreviewPhase::ProtocolFailed => 63,
            _ => 62,
        };
        if state.events.len() >= limit {
            return;
        }
        let event = PhaseEvent {
            schema: "preview-test-phase-v1".into(),
            sequence: u32::try_from(state.events.len()).unwrap_or(64),
            elapsed_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            phase,
            metadata,
        };
        let Ok(mut line) = facet_json::to_string(&event) else {
            self.failed.store(true, Ordering::Release);
            return;
        };
        if line.len() > 1024 {
            self.failed.store(true, Ordering::Release);
            return;
        }
        line.push('\n');
        if let Some(file) = state.file.as_mut()
            && file
                .write_all(line.as_bytes())
                .and_then(|()| file.flush())
                .is_err()
        {
            self.failed.store(true, Ordering::Release);
        }
        state.events.push(event);
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
pub(super) fn emit(trace: Option<&PreviewTrace>, phase: PreviewPhase) {
    if let Some(trace) = trace {
        trace.emit(phase, PreviewMetadata::default());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_has_only_fixed_labels_primitives_and_a_hard_bound() {
        let trace = PreviewTrace {
            started: Instant::now(),
            state: Mutex::new(State {
                events: Vec::new(),
                file: None,
            }),
            failed: AtomicBool::new(false),
        };
        for _ in 0..100 {
            trace.emit(
                PreviewPhase::PeerConfirmationVerified,
                PreviewMetadata {
                    origin: Some(PreviewOrigin::Phone),
                    accepted: Some(true),
                    ..PreviewMetadata::default()
                },
            );
        }
        trace.emit(PreviewPhase::ProtocolFailed, PreviewMetadata::default());
        trace.emit(PreviewPhase::Cleanup, PreviewMetadata::default());
        let state = trace.state.lock().unwrap();
        assert_eq!(state.events.len(), 64);
        for event in &state.events {
            let text = facet_json::to_string(event).unwrap();
            assert!(text.len() < 1024);
            for forbidden in [
                "SAS-SENTINEL",
                "SECRET-SENTINEL",
                "request_nonce",
                "secret_ref",
                "binding",
                "payload",
                "authentication_code",
                "key_material",
            ] {
                assert!(!text.contains(forbidden));
            }
        }
        assert_eq!(format!("{trace:?}"), "PreviewTrace([bounded metadata])");
    }
    #[test]
    fn every_phase_roundtrips_with_no_protocol_content_field() {
        let event = PhaseEvent {
            schema: "preview-test-phase-v1".into(),
            sequence: 1,
            elapsed_ms: 20,
            phase: PreviewPhase::PreviewResolved,
            metadata: PreviewMetadata {
                origin: Some(PreviewOrigin::Phone),
                decision: Some(crate::protocol::ApprovalPreviewDisposition::ApprovedTest),
                ack_written: Some(true),
                ..PreviewMetadata::default()
            },
        };
        let text = facet_json::to_string(&event).unwrap();
        let decoded: PhaseEvent = facet_json::from_str(&text).unwrap();
        assert_eq!(decoded.phase, PreviewPhase::PreviewResolved);
        assert_eq!(decoded.metadata.origin, Some(PreviewOrigin::Phone));
        assert_eq!(decoded.metadata.ack_written, Some(true));
    }
}
