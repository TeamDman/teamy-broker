use super::Controller;
use crate::protocol::phone_status::PhoneFailure;
use crate::protocol::phone_status::PhoneFailureCategory;
use crate::protocol::phone_status::PhonePhase;
use std::sync::Arc;
impl Controller {
    pub(crate) fn phase(&self, phase: PhonePhase) {
        if let Ok(mut state) = self.diagnostic.lock() {
            state.phase = phase;
            state.phase_started_unix_ms = crate::protocol::wire::unix_ms().ok();
        }
    }
    pub(crate) fn failure(&self, category: PhoneFailureCategory) {
        if let Ok(mut state) = self.diagnostic.lock() {
            state.last_failure = Some(PhoneFailure {
                phase: state.phase,
                category,
                attempt: state.attempts,
                transport_kind: None,
                transport_os_code: None,
                elapsed_phase_ms: crate::protocol::wire::unix_ms()
                    .ok()
                    .zip(state.phase_started_unix_ms)
                    .and_then(|(now, start)| now.checked_sub(start)),
            });
        }
    }
    pub(crate) fn transport_failure(&self, error: &std::io::Error) {
        use crate::protocol::phone_status::PhoneTransportKind as K;
        self.failure(PhoneFailureCategory::ReadTransport);
        if let Ok(mut state) = self.diagnostic.lock()
            && let Some(failure) = &mut state.last_failure
        {
            failure.transport_os_code = error.raw_os_error();
            failure.transport_kind = Some(match error.kind() {
                std::io::ErrorKind::ConnectionReset => K::ConnectionReset,
                std::io::ErrorKind::ConnectionAborted => K::ConnectionAborted,
                std::io::ErrorKind::BrokenPipe => K::BrokenPipe,
                std::io::ErrorKind::TimedOut => K::TimedOut,
                std::io::ErrorKind::UnexpectedEof => K::UnexpectedEof,
                std::io::ErrorKind::NotConnected => K::NotConnected,
                _ => K::Other,
            });
        }
    }
}
pub(super) struct Attempt {
    controller: Arc<Controller>,
    completed: bool,
}
impl Attempt {
    pub(super) fn begin(controller: Arc<Controller>) -> Self {
        if let Ok(mut state) = controller.diagnostic.lock() {
            state.phase = PhonePhase::Accepted;
            state.attempts = state.attempts.saturating_add(1);
        }
        Self {
            controller,
            completed: false,
        }
    }
    pub(super) fn finish(mut self, success: bool) {
        self.completed = true;
        if success {
            self.controller.phase(PhonePhase::Closed);
        } else if let Ok(mut state) = self.controller.diagnostic.lock()
            && state
                .last_failure
                .is_none_or(|failure| failure.attempt != state.attempts)
        {
            state.last_failure = Some(PhoneFailure {
                phase: state.phase,
                category: PhoneFailureCategory::Unavailable,
                attempt: state.attempts,
                transport_kind: None,
                transport_os_code: None,
                elapsed_phase_ms: crate::protocol::wire::unix_ms()
                    .ok()
                    .zip(state.phase_started_unix_ms)
                    .and_then(|(now, start)| now.checked_sub(start)),
            });
        }
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.completed {
            self.controller.failure(PhoneFailureCategory::Deadline);
        }
    }
}
pub(super) fn tls_category(error: &std::io::Error) -> PhoneFailureCategory {
    use rustls::CertificateError;
    use rustls::Error;
    match error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Error>())
    {
        Some(Error::NoCertificatesPresented) => PhoneFailureCategory::ClientCertificateAbsent,
        Some(Error::InvalidCertificate(reason)) => match reason {
            CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
                PhoneFailureCategory::CertificateExpired
            }
            CertificateError::UnknownIssuer => PhoneFailureCategory::CertificateUnknownIssuer,
            CertificateError::BadSignature => PhoneFailureCategory::CertificateBadSignature,
            CertificateError::InvalidPurpose | CertificateError::InvalidPurposeContext { .. } => {
                PhoneFailureCategory::CertificateInvalidPurpose
            }
            _ => PhoneFailureCategory::CertificateRejected,
        },
        Some(Error::AlertReceived(_)) => PhoneFailureCategory::PeerAlert,
        Some(_) => PhoneFailureCategory::TlsProtocol,
        None => PhoneFailureCategory::TlsTransport,
    }
}
