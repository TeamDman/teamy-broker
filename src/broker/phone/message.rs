use facet::Facet;
#[derive(Facet)]
pub(super) struct Message {
    pub version: u32,
    pub scope: String,
    pub session_id: String,
    pub sequence: u64,
    pub operation: String,
    pub request_id: Option<String>,
    pub decision: Option<String>,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PhoneMessage([no-vault])")
    }
}
impl Message {
    pub fn validate(&self, session: &str, sequence: u64) -> eyre::Result<()> {
        eyre::ensure!(
            self.version == 1
                && self.scope == super::SCOPE
                && self.session_id == session
                && self.sequence == sequence,
            "phone protocol rejected"
        );
        match self.operation.as_str() {
            "poll" => eyre::ensure!(
                self.request_id.is_none() && self.decision.is_none(),
                "phone protocol rejected"
            ),
            "supply" => {
                uuid::Uuid::parse_str(
                    self.request_id
                        .as_deref()
                        .ok_or_else(|| eyre::eyre!("missing request ID"))?,
                )?;
                eyre::ensure!(self.decision.is_none(), "phone operation rejected");
            }
            "decide" => {
                uuid::Uuid::parse_str(
                    self.request_id
                        .as_deref()
                        .ok_or_else(|| eyre::eyre!("missing preview ID"))?,
                )?;
                eyre::ensure!(
                    matches!(
                        self.decision.as_deref(),
                        Some("approve" | "deny" | "cancel")
                    ),
                    "phone operation rejected"
                );
            }
            _ => eyre::bail!("phone operation rejected"),
        }
        Ok(())
    }
}
#[derive(Facet, Debug)]
pub(super) struct Reply {
    pub version: u32,
    pub scope: String,
    pub session_id: String,
    pub sequence: u64,
    pub pending: Option<crate::protocol::SharedPreviewSnapshot>,
    pub accepted: Option<bool>,
}
