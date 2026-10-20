#[derive(facet::Facet, Debug)]
pub struct PhoneStatus {
    pub paired: bool,
    pub pairing_window_open: bool,
    #[facet(default)]
    pub pairing_attempt_id: Option<String>,
    pub listener: bool,
    pub connection: PhoneConnectionState,
    pub runtime: PhoneRuntimeState,
    pub diagnostic: PhoneDiagnostic,
    pub scope: String,
    pub peer: String,
}

#[derive(facet::Facet, Debug, PartialEq)]
#[repr(u8)]
pub enum PhoneConnectionState {
    Disconnected,
    Connected,
}

#[derive(facet::Facet, Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum PhoneRuntimeState {
    Stopped,
    Running,
    Recovering,
    Failed,
}

/// Fixed categories only; no identities, frames, certificates, errors or protocol values.
#[derive(facet::Facet, Debug, Clone, Copy, Default)]
pub struct PhoneDiagnostic {
    pub phase: PhonePhase,
    pub last_failure: Option<PhoneFailure>,
    pub attempts: u64,
    #[facet(default)]
    pub phase_started_unix_ms: Option<u64>,
}
#[derive(facet::Facet, Debug, Clone, Copy)]
pub struct PhoneFailure {
    pub phase: PhonePhase,
    pub category: PhoneFailureCategory,
    pub attempt: u64,
    #[facet(default)]
    pub transport_kind: Option<PhoneTransportKind>,
    #[facet(default)]
    pub transport_os_code: Option<i32>,
    #[facet(default)]
    pub elapsed_phase_ms: Option<u64>,
}
#[derive(facet::Facet, Debug, Clone, Copy, Default, PartialEq)]
#[repr(u8)]
pub enum PhonePhase {
    #[default]
    Idle,
    Accepted,
    PeerVerification,
    RelationshipLoad,
    Pairing,
    PairingHandshake,
    PairingComparison,
    PairingLocalConfirmation,
    PairingPeerConfirmation,
    PairingBootstrap,
    IdentityEnrollment,
    TlsConfiguration,
    TlsHandshake,
    CertificateAuthorization,
    HelloWrite,
    MessageRead,
    MessageValidation,
    PeerReverification,
    ReplyWrite,
    Closed,
}
#[derive(facet::Facet, Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum PhoneFailureCategory {
    Unavailable,
    Deadline,
    TlsTransport,
    ReadClosed,
    ReadTransport,
    MessageRejected,
    TlsProtocol,
    ClientCertificateAbsent,
    CertificateRejected,
    CertificateExpired,
    CertificateUnknownIssuer,
    CertificateBadSignature,
    CertificateInvalidPurpose,
    PeerAlert,
    PairingUiUnavailable,
    PairingRejected,
}

#[derive(facet::Facet, Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum PhoneTransportKind {
    ConnectionReset,
    ConnectionAborted,
    BrokenPipe,
    TimedOut,
    UnexpectedEof,
    NotConnected,
    Other,
}
