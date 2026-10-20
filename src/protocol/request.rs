use facet::Facet;
#[derive(Facet)]
pub struct Request {
    pub operation: String,
    pub acquire: Option<super::AcquireRequest>,
    pub lease_id: Option<String>,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Request([redacted])")
    }
}
impl Request {
    #[must_use]
    pub fn control(operation: &str, lease_id: Option<String>) -> Self {
        Self {
            operation: operation.into(),
            acquire: None,
            lease_id,
        }
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn validate(&self) -> eyre::Result<()> {
        match self.operation.as_str() {
            "acquire"
            | "acquire-fixture"
            | "phone-authorize-desktop"
            | "phone-authorize-supply"
            | "phone-authorize-fixture" => {
                eyre::ensure!(self.lease_id.is_none(), "invalid request shape");
                self.acquire
                    .as_ref()
                    .ok_or_else(|| eyre::eyre!("missing acquire"))?
                    .validate()
            }
            "status" | "stop" | "stop-force" | "approval-preview" | "preview-state"
            | "phone-pair" | "phone-revoke" | "phone-list" | "phone-status" => {
                eyre::ensure!(
                    self.acquire.is_none() && self.lease_id.is_none(),
                    "invalid request shape"
                );
                Ok(())
            }
            "phone-pair-inspect" | "phone-pair-confirm" | "phone-pair-cancel"
            | "preview-approve" | "preview-deny" | "preview-cancel" => {
                eyre::ensure!(self.acquire.is_none(), "invalid request shape");
                uuid::Uuid::parse_str(
                    self.lease_id
                        .as_deref()
                        .ok_or_else(|| eyre::eyre!("missing preview ID"))?,
                )
                .map_err(|_redacted_error| eyre::eyre!("invalid preview ID"))?;
                Ok(())
            }
            "purge" | "validate" => {
                eyre::ensure!(self.acquire.is_none(), "invalid request shape");
                eyre::ensure!(
                    self.operation != "validate" || self.lease_id.is_some(),
                    "missing lease ID"
                );
                if let Some(id) = &self.lease_id {
                    uuid::Uuid::parse_str(id)
                        .map_err(|_redacted_error| eyre::eyre!("invalid opaque lease ID"))?;
                }
                Ok(())
            }
            _ => eyre::bail!("unsupported operation"),
        }
    }
}
