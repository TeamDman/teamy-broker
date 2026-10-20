use facet::Facet;

/// Canonical serialized bytes are AEAD associated data. Never rebuild them on the phone.
#[derive(Facet, Clone, PartialEq, Eq)]
pub struct Binding {
    pub version: u32,
    pub request_id: String,
    pub request_nonce: String,
    pub desktop: String,
    pub phone: String,
    pub consumer: String,
    pub consumer_pid: u32,
    pub account: String,
    pub reference: String,
    pub purpose: String,
    pub destination: String,
    pub scope: String,
    pub expires_unix_ms: u64,
    pub lease_ms: u64,
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Binding([redacted])")
    }
}
impl Binding {
    pub fn dummy(pid: u32, now: u64) -> eyre::Result<Self> {
        Ok(Self {
            version: 1,
            request_id: crate::wire::hex(&crate::wire::random::<16>()?),
            request_nonce: crate::wire::hex(&crate::wire::random::<32>()?),
            desktop: crate::DESKTOP.into(),
            phone: crate::PHONE.into(),
            consumer: crate::CONSUMER.into(),
            consumer_pid: pid,
            account: "synthetic-account".into(),
            reference: crate::REFERENCE.into(),
            purpose: crate::PURPOSE.into(),
            destination: "https://example.invalid".into(),
            scope: "memory-only".into(),
            expires_unix_ms: now
                .checked_add(crate::REQUEST_MS)
                .ok_or_else(|| eyre::eyre!("invalid clock"))?,
            lease_ms: crate::LEASE_MS,
        })
    }
    pub fn encode(&self) -> eyre::Result<String> {
        facet_json::to_string(self).map_err(|_redacted_error| eyre::eyre!("invalid binding"))
    }
}
