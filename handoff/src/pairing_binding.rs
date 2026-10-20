use facet::Facet;
/// Exact canonical application context, echoed under UKEY2-derived authentication on both sides.
/// Initial bytes are untrusted until both live-channel confirmations validate them.
#[derive(Facet, Clone, PartialEq, Eq)]
pub struct PairingBinding {
    pub version: u32,
    pub session_id: String,
    pub nonce: String,
    pub desktop: String,
    pub desktop_node: String,
    pub phone: String,
    pub phone_node: String,
    pub endpoint: String,
    pub scope: String,
    pub pairing_expires_unix_ms: u64,
    pub session_expires_unix_ms: u64,
}
impl std::fmt::Debug for PairingBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingBinding([redacted])")
    }
}
impl PairingBinding {
    /// # Errors
    /// Rejects invalid clocks/randomness; supports only the inspected synthetic peer scope.
    pub fn dummy(now: u64) -> eyre::Result<Self> {
        Ok(Self {
            version: 2,
            session_id: crate::wire::hex(&crate::wire::random::<16>()?),
            nonce: crate::wire::hex(&crate::wire::random::<32>()?),
            desktop: crate::DESKTOP.into(),
            desktop_node: crate::phone_config::DESKTOP_NODE.into(),
            phone: crate::PHONE.into(),
            phone_node: crate::phone_config::PHONE_NODE.into(),
            endpoint: format!(
                "{}:{}",
                crate::phone_config::DESKTOP,
                crate::phone_config::PORT
            ),
            scope: "dummy-only/session-only/ukey2-p256".into(),
            pairing_expires_unix_ms: now
                .checked_add(90_000)
                .ok_or_else(|| eyre::eyre!("invalid clock"))?,
            session_expires_unix_ms: now
                .checked_add(180_000)
                .ok_or_else(|| eyre::eyre!("invalid clock"))?,
        })
    }
    /// # Errors
    /// Returns a static redacted error if serialization fails.
    pub fn encode(&self) -> eyre::Result<String> {
        facet_json::to_string(self)
            .map_err(|_redacted_error| eyre::eyre!("pairing context rejected"))
    }
}
