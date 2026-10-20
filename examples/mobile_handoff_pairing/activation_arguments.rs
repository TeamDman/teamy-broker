use facet::Facet;
use std::ffi::OsString;
/// Action-time readiness is distinct from approval of this fixed dummy scope.
#[derive(Debug, Facet, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum PairingActivation {
    Proposal,
    Ready,
}
impl PairingActivation {
    pub fn parse(arguments: &[OsString]) -> eyre::Result<Self> {
        if arguments.is_empty() {
            return Ok(Self::Proposal);
        }
        eyre::ensure!(
            arguments
                == [
                    OsString::from("--activate-dummy-pairing"),
                    OsString::from("--phone-ready")
                ],
            "explicit dummy activation and phone readiness required"
        );
        Ok(Self::Ready)
    }
}
#[cfg(test)]
#[path = "activation_arguments_tests.rs"]
mod tests;
