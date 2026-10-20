use super::preview_phase::PreviewTrace;
use facet::Facet;
use sha2::Digest;
use sha2::Sha256;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
/// Same-user action-time audit authorization, not device trust or cryptographic enrollment.
#[derive(Facet)]
struct ApprovalReceipt {
    version: u32,
    approval_id: String,
    scope: String,
    desktop: String,
    phone: String,
    phone_node: String,
    pairing_seconds: u32,
    total_seconds: u32,
    request_seconds: u32,
    approved_at_unix_ms: u64,
    start_before_unix_ms: u64,
    candidate_sha256: String,
}
/// Only the bounded preview example constructs this after consuming its exact one-use receipt.
pub struct PreviewActivation {
    start_before: u64,
    trace: Arc<PreviewTrace>,
}
impl std::fmt::Debug for PreviewActivation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreviewActivation([one bounded attempt])")
    }
}
impl PreviewActivation {
    /// # Errors
    /// Rejects missing, stale, reused, wrong-artifact or broadened-scope same-user receipts.
    pub fn consume(receipt_path: &Path, phase_path: &Path) -> eyre::Result<Self> {
        let now = crate::protocol::wire::unix_ms()?;
        let executable = std::env::current_exe()
            .map_err(|_redacted_error| eyre::eyre!("candidate identity unavailable"))?;
        let bytes = std::fs::read(executable)
            .map_err(|_redacted_error| eyre::eyre!("candidate identity unavailable"))?;
        let hash = format!("{:X}", Sha256::digest(bytes));
        let start_before = consume_checked(receipt_path, now, &hash)?;
        let trace = Arc::new(PreviewTrace::create(phase_path)?);
        Ok(Self {
            start_before,
            trace,
        })
    }
    /// # Errors
    /// A receipt permits prompt launch only within its nonrenewable two-minute action-time window.
    pub fn require_live(&self) -> eyre::Result<()> {
        eyre::ensure!(
            crate::protocol::wire::unix_ms()? < self.start_before && !self.trace.failed(),
            "preview activation expired or diagnostics unavailable"
        );
        Ok(())
    }
    #[must_use]
    pub fn trace(&self) -> Arc<PreviewTrace> {
        Arc::clone(&self.trace)
    }
}
fn consumed_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".consumed");
    PathBuf::from(name)
}
fn consume_checked(path: &Path, now: u64, hash: &str) -> eyre::Result<u64> {
    let mut file = std::fs::File::open(path)
        .map_err(|_redacted_error| eyre::eyre!("preview approval receipt unavailable"))?;

    let mut bytes = Vec::new();
    (&mut file)
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_redacted_error| eyre::eyre!("preview approval receipt unavailable"))?;
    eyre::ensure!(bytes.len() <= 4096, "preview approval receipt rejected");
    let receipt: ApprovalReceipt = facet_json::from_str(
        std::str::from_utf8(&bytes)
            .map_err(|_redacted_error| eyre::eyre!("preview approval receipt rejected"))?,
    )
    .map_err(|_redacted_error| eyre::eyre!("preview approval receipt rejected"))?;
    eyre::ensure!(
        receipt.version == 1
            && uuid::Uuid::parse_str(&receipt.approval_id).is_ok()
            && receipt.scope == "resident-no-vault-preview-v1"
            && receipt.desktop
                == format!(
                    "{}:{}",
                    super::tailnet_peer::DESKTOP,
                    super::tailnet_peer::PORT
                )
            && receipt.phone == super::tailnet_peer::PHONE.to_string()
            && receipt.phone_node == super::tailnet_peer::PHONE_NODE
            && receipt.pairing_seconds == 90
            && receipt.total_seconds == 180
            && receipt.request_seconds == 60
            && receipt.candidate_sha256 == hash
            && receipt.approved_at_unix_ms <= now
            && now < receipt.start_before_unix_ms
            && receipt
                .start_before_unix_ms
                .checked_sub(receipt.approved_at_unix_ms)
                .is_some_and(|window| window <= 120_000),
        "preview approval receipt rejected"
    );
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(consumed_path(path))
        .map_err(|_redacted_error| {
            eyre::eyre!("preview approval receipt already consumed or unavailable")
        })?;
    Ok(receipt.start_before_unix_ms)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir()
                .join(format!("teamy-preview-approval-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&dir).unwrap();
            Self(dir.join("approval.json"))
        }
        fn write(&self, scope: &str, start_before: u64, hash: &str) {
            let receipt = ApprovalReceipt {
                version: 1,
                approval_id: uuid::Uuid::new_v4().to_string(),
                scope: scope.into(),
                desktop: format!(
                    "{}:{}",
                    super::super::tailnet_peer::DESKTOP,
                    super::super::tailnet_peer::PORT
                ),
                phone: super::super::tailnet_peer::PHONE.to_string(),
                phone_node: super::super::tailnet_peer::PHONE_NODE.into(),
                pairing_seconds: 90,
                total_seconds: 180,
                request_seconds: 60,
                approved_at_unix_ms: 1000,
                start_before_unix_ms: start_before,
                candidate_sha256: hash.into(),
            };
            std::fs::write(&self.0, facet_json::to_string(&receipt).unwrap()).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(consumed_path(&self.0));
            if let Some(dir) = self.0.parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
    #[test]
    fn action_time_scope_artifact_expiry_and_replay_fail_closed() {
        let fixture = Fixture::new();
        fixture.write("wrong-scope", 2000, "A");
        drop(consume_checked(&fixture.0, 1001, "A").unwrap_err());
        fixture.write("resident-no-vault-preview-v1", 2000, "A");
        drop(consume_checked(&fixture.0, 2000, "A").unwrap_err());
        drop(consume_checked(&fixture.0, 1001, "B").unwrap_err());
        assert_eq!(consume_checked(&fixture.0, 1001, "A").unwrap(), 2000);
        drop(consume_checked(&fixture.0, 1001, "A").unwrap_err());
    }
    #[test]
    fn concurrent_receipt_consumption_has_one_winner() {
        let fixture = Fixture::new();
        fixture.write("resident-no-vault-preview-v1", 2000, "A");
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = fixture.0.clone();
                std::thread::spawn(move || consume_checked(&path, 1001, "A").is_ok())
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .filter(|accepted| *accepted)
                .count(),
            1
        );
    }
}
