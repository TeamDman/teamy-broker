use crate::protocol::AcquireRequest;
use crate::protocol::Backend;
pub const INPUT: &[u8] = b"teamy-fixture-input-v1";
pub const PROVIDER: &[u8] = b"teamy-fixture-provider-v1";
#[must_use]
pub fn request() -> AcquireRequest {
    AcquireRequest {
        account: "isolated-fixture-v1".into(),
        secret_ref: "op://Fixture/Approval/input".into(),
        requesting_app: "teamy-broker-fixture-v1".into(),
        purpose: "Isolated synthetic approval acceptance".into(),
        destination: "fixture://local-no-network-v1".into(),
        backend: Backend::Memory,
    }
}
/// # Errors
/// Rejects all metadata except the built-in isolated fixture.
pub fn validate(candidate: &AcquireRequest) -> eyre::Result<()> {
    eyre::ensure!(
        *candidate == request(),
        "isolated fixture metadata required"
    );
    Ok(())
}
/// # Errors
/// Rejects changed fixture metadata. Never invokes an adapter, op or network.
pub fn read(candidate: &AcquireRequest) -> eyre::Result<zeroize::Zeroizing<Vec<u8>>> {
    validate(candidate)?;
    Ok(zeroize::Zeroizing::new(PROVIDER.to_vec()))
}
